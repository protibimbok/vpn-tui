//! Row selection, scroll window, and mouse clicks.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::{Action, Store, api::Server};

use super::ServerList;

impl ServerList {
    /// Keep `scroll` such that the selected row stays inside a window of `page_size`.
    pub fn sync_scroll(&mut self, page_size: usize) {
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

    pub fn selected<'a>(&self, state: &'a Store) -> Option<&'a Server> {
        let row = self.table_state.selected()?;
        let index = *self.visible.get(row)?;
        state.servers.get(index)
    }

    pub fn move_by(&mut self, delta: i32) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, self.visible.len() as i32 - 1);
        self.table_state.select(Some(next as usize));
    }

    pub fn select_first(&mut self) {
        if !self.visible.is_empty() {
            self.table_state.select(Some(0));
            self.scroll = 0;
        }
    }

    pub fn select_last(&mut self) {
        if !self.visible.is_empty() {
            let last = self.visible.len() - 1;
            self.table_state.select(Some(last));
        }
    }

    pub fn handle_mouse(&mut self, evt: MouseEvent) -> Action {
        match evt.kind {
            MouseEventKind::ScrollUp => {
                self.move_by(-1);
                Action::Ignore
            }
            MouseEventKind::ScrollDown => {
                self.move_by(1);
                Action::Ignore
            }
            MouseEventKind::Down(MouseButton::Left) => self.select_at_mouse(evt),
            _ => Action::None,
        }
    }

    fn select_at_mouse(&mut self, evt: MouseEvent) -> Action {
        if self.visible.is_empty() || self.body_area.height == 0 {
            return Action::Ignore;
        }
        let body = self.body_area;
        if evt.column < body.x
            || evt.column >= body.x.saturating_add(body.width)
            || evt.row < body.y
            || evt.row >= body.y.saturating_add(body.height)
        {
            return Action::Ignore;
        }
        let local = (evt.row - body.y) as usize;
        let row = self.scroll + local;
        if row >= self.visible.len() {
            return Action::Ignore;
        }
        self.table_state.select(Some(row));
        Action::Ignore
    }
}
