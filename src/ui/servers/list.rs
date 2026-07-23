//! Filter, sort, and row selection for the servers table.

use ratatui::widgets::TableState;

use crate::{Action, Store, api::Server};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SortMode {
    Title,
    Load,
    Latency,
}

impl SortMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            SortMode::Title => "title",
            SortMode::Load => "load",
            SortMode::Latency => "latency",
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            SortMode::Title => SortMode::Load,
            SortMode::Load => SortMode::Latency,
            SortMode::Latency => SortMode::Title,
        }
    }
}

/// Column layout chosen from terminal width.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Density {
    Compact,
    Comfortable,
    Wide,
}

impl Density {
    pub(super) fn from_width(width: u16) -> Self {
        if width < 72 {
            Density::Compact
        } else if width < 110 {
            Density::Comfortable
        } else {
            Density::Wide
        }
    }
}

pub(super) struct ServerList {
    pub filter: String,
    pub pending_filter_closing: bool,
    pub filtering: bool,
    pub sort: SortMode,
    pub table_state: TableState,
    /// Indices into `Store::servers` after filter/sort.
    pub visible: Vec<usize>,
    /// First visible index into `visible` (scroll window start).
    pub scroll: usize,
    /// Avoid rebuilding/sorting the index list every 60 Hz frame.
    recompute_key: RecomputeKey,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RecomputeKey {
    servers_epoch: u64,
    latency_count: usize,
    sort: SortMode,
    filter_len: usize,
    /// Cheap fingerprint of the filter string.
    filter_hash: u64,
}

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
    pub(super) fn new() -> Self {
        Self {
            filter: String::new(),
            pending_filter_closing: false,
            filtering: false,
            sort: SortMode::Title,
            table_state: TableState::default().with_selected(Some(0)),
            visible: Vec::new(),
            scroll: 0,
            recompute_key: RecomputeKey {
                servers_epoch: u64::MAX, // force first recompute
                latency_count: 0,
                sort: SortMode::Title,
                filter_len: 0,
                filter_hash: 0,
            },
        }
    }

    pub(super) fn recompute(&mut self, state: &Store) {
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

    /// Keep `scroll` such that the selected row stays inside a window of `page_size`.
    pub(super) fn sync_scroll(&mut self, page_size: usize) {
        let page_size = page_size.max(1);
        let Some(selected) = self.table_state.selected() else {
            self.scroll = 0;
            return;
        };
        if selected < self.scroll {
            self.scroll = selected;
        } else if selected >= self.scroll.saturating_add(page_size) {
            self.scroll = selected + 1 - page_size;
        }
        let max_scroll = self.visible.len().saturating_sub(page_size);
        self.scroll = self.scroll.min(max_scroll);
    }

    pub(super) fn selected<'a>(&self, state: &'a Store) -> Option<&'a Server> {
        let row = self.table_state.selected()?;
        let index = *self.visible.get(row)?;
        state.servers.get(index)
    }

    pub(super) fn move_by(&mut self, delta: i32) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, self.visible.len() as i32 - 1);
        self.table_state.select(Some(next as usize));
    }

    pub(super) fn select_first(&mut self) {
        if !self.visible.is_empty() {
            self.table_state.select(Some(0));
            self.scroll = 0;
        }
    }

    pub(super) fn select_last(&mut self) {
        if !self.visible.is_empty() {
            let last = self.visible.len() - 1;
            self.table_state.select(Some(last));
        }
    }

    pub(super) fn connect_fastest(state: &Store) -> Action {
        let fastest = state
            .servers
            .iter()
            .filter_map(|s| {
                state
                    .latencies
                    .get(&s.endpoint_host)
                    .copied()
                    .flatten()
                    .map(|ms| (ms, s))
            })
            .min_by_key(|(ms, _)| *ms)
            .map(|(_, s)| s.clone());
        let target = fastest.or_else(|| state.servers.iter().min_by_key(|s| s.load).cloned());
        match target {
            Some(server) => Action::Connect(server),
            None => Action::Error("no servers loaded".into()),
        }
    }

    pub(super) fn is_connected(state: &Store, s: &Server) -> bool {
        let Some(c) = state.connected.as_ref() else {
            return false;
        };
        // Unique logical id: Proton `NL#54`, Surfshark connection hostname.
        if c == &s.name || c == &s.connected_label() {
            return true;
        }
        // Peer pubkey uniquely identifies the physical tunnel (Proton EntryIPs
        // are shared across many logicals; city/country alone is not unique).
        if let Some(pk) = crate::utils::wg::conf_peer_public_key(&crate::utils::conf_path()) {
            return pk == s.wg_public_key;
        }
        // Legacy label / endpoint-only restore when conf isn't readable.
        c == &s.display_name() || c.contains(&s.endpoint_host)
    }
}

