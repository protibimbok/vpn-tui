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
    pub filtering: bool,
    pub sort: SortMode,
    pub table_state: TableState,
    pub visible: Vec<usize>,
}

impl ServerList {
    pub(super) fn new() -> Self {
        Self {
            filter: String::new(),
            filtering: false,
            sort: SortMode::Title,
            table_state: TableState::default().with_selected(Some(0)),
            visible: Vec::new(),
        }
    }

    pub(super) fn recompute(&mut self, state: &Store) {
        let needle = self.filter.to_lowercase();
        let terms: Vec<&str> = needle.split_whitespace().collect();
        self.visible = state
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                terms.is_empty() || {
                    let haystack =
                        format!("{} {} {}", s.country, s.location, s.name).to_lowercase();
                    terms.iter().all(|t| haystack.contains(t))
                }
            })
            .map(|(i, _)| i)
            .collect();

        let servers = &state.servers;
        let latencies = &state.latencies;
        match self.sort {
            SortMode::Title => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| {
                    (
                        servers[i].country.as_str(),
                        servers[i].location.as_str(),
                        servers[i].name.as_str(),
                    )
                };
                key(a).cmp(&key(b))
            }),
            SortMode::Load => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| (servers[i].load, servers[i].country.as_str());
                key(a).cmp(&key(b))
            }),
            SortMode::Latency => self.visible.sort_by(|&a, &b| {
                let key = |i: usize| {
                    let ms = latencies
                        .get(&servers[i].endpoint_host)
                        .copied()
                        .flatten()
                        .unwrap_or(u32::MAX);
                    (ms, servers[i].load)
                };
                key(a).cmp(&key(b))
            }),
        }

        if self.visible.is_empty() {
            self.table_state.select(None);
        } else {
            let row = self.table_state.selected().unwrap_or(0);
            self.table_state
                .select(Some(row.min(self.visible.len() - 1)));
        }
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
        }
    }

    pub(super) fn select_last(&mut self) {
        if !self.visible.is_empty() {
            self.table_state.select(Some(self.visible.len() - 1));
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
        state.connected.as_ref().is_some_and(|c| {
            c == &s.display_name() || c == &s.name || c.contains(&s.endpoint_host)
        })
    }
}
