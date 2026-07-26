//! Row selection, scroll window, and mouse clicks.

use crate::{Store, api::Server};

use super::ServerList;

impl ServerList {
    /// Keep `scroll` such that the selected row stays inside a window of `page_size`.
    pub(in crate::ui::servers) fn sync_scroll(&mut self, page_size: usize) {
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

    pub(in crate::ui::servers) fn selected<'a>(&self, state: &'a Store) -> Option<&'a Server> {
        let row = self.table_state.selected()?;
        let index = *self.visible.get(row)?;
        state.servers.get(index)
    }

    pub(in crate::ui::servers) fn move_by(&mut self, delta: i32) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, self.visible.len() as i32 - 1);
        self.table_state.select(Some(next as usize));
    }

    pub(in crate::ui::servers) fn select_first(&mut self) {
        if !self.visible.is_empty() {
            self.table_state.select(Some(0));
            self.scroll = 0;
        }
    }

    pub(in crate::ui::servers) fn select_last(&mut self) {
        if !self.visible.is_empty() {
            let last = self.visible.len() - 1;
            self.table_state.select(Some(last));
        }
    }
}
