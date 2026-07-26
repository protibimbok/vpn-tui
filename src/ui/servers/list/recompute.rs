//! Filter/sort index rebuild for the visible server rows.

use crate::Store;

use super::{RecomputeKey, ServerList, SortMode};

impl RecomputeKey {
    fn from(list: &ServerList, state: &Store) -> Self {
        Self {
            servers_epoch: state.servers_epoch,
            latency_count: state.latencies.len(),
            sort: list.sort,
            filter_len: list.filter.len(),
            filter_hash: {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                list.filter.hash(&mut h);
                h.finish()
            },
        }
    }
}

impl ServerList {
    pub(in crate::ui::servers) fn recompute(&mut self, state: &Store) {
        let key = RecomputeKey::from(self, state);
        if key == self.recompute_key {
            if let Some(row) = self.table_state.selected()
                && !self.visible.is_empty()
            {
                self.table_state
                    .select(Some(row.min(self.visible.len() - 1)));
            }
            return;
        }
        self.recompute_key = key;

        let needle = self.filter.to_lowercase();
        let terms: Vec<&str> = needle.split_whitespace().collect();
        self.visible = state
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                terms.is_empty() || {
                    let haystack = s.connected_label().to_lowercase();
                    terms.iter().all(|t| haystack.contains(t))
                }
            })
            .map(|(i, _)| i)
            .collect();

        let servers = &state.servers;
        let latencies = &state.latencies;
        // 0 = country matches all terms, 1 = any term, 2 = only location/id.
        let country_rank = |i: usize| -> u8 {
            if terms.is_empty() {
                return 0;
            }
            let country = servers[i].country.to_lowercase();
            if terms.iter().all(|t| country.contains(t)) {
                0
            } else if terms.iter().any(|t| country.contains(t)) {
                1
            } else {
                2
            }
        };
        match self.sort {
            SortMode::Title => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| {
                    (
                        country_rank(i),
                        servers[i].country.as_str(),
                        servers[i].location.as_str(),
                        servers[i].name.as_str(),
                    )
                };
                key(a).cmp(&key(b))
            }),
            SortMode::Load => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| (country_rank(i), servers[i].load, servers[i].country.as_str());
                key(a).cmp(&key(b))
            }),
            SortMode::Latency => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| {
                    let ms = latencies
                        .get(&servers[i].endpoint_host)
                        .copied()
                        .flatten()
                        .unwrap_or(u32::MAX);
                    (country_rank(i), ms, servers[i].load)
                };
                key(a).cmp(&key(b))
            }),
        }

        if self.visible.is_empty() {
            self.table_state.select(None);
            self.scroll = 0;
        } else {
            let row = self.table_state.selected().unwrap_or(0);
            self.table_state
                .select(Some(row.min(self.visible.len() - 1)));
            self.scroll = self.scroll.min(self.visible.len().saturating_sub(1));
        }
    }
}
