//! Filter, sort, and row selection for the servers table.

mod connect;
mod recompute;
mod selection;
mod types;

use std::time::Instant;

use ratatui::{layout::Rect, widgets::TableState};

pub(super) use types::{Density, SortMode};

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
    /// Screen rect of table body rows from last render (for mouse hit-testing).
    pub body_area: Rect,
    /// Last left-click `(time, visible-row)` for double-click connect.
    last_click: Option<(Instant, usize)>,
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
            body_area: Rect::default(),
            last_click: None,
            recompute_key: RecomputeKey {
                servers_epoch: u64::MAX, // force first recompute
                latency_count: 0,
                sort: SortMode::Title,
                filter_len: 0,
                filter_hash: 0,
            },
        }
    }
}
