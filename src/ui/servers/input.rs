//! Keyboard handling for the servers screen.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{Action, Store};

use super::list::{ServerList, SortMode};

impl ServerList {
    pub(super) fn handle_key(&mut self, event: KeyEvent, state: &Store) -> Action {
        if self.filtering {
            return self.handle_search_key(event, state);
        }

        match event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_by(-1);
                Action::Ignore
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_by(1);
                Action::Ignore
            }
            KeyCode::PageUp => {
                self.move_by(-10);
                Action::Ignore
            }
            KeyCode::PageDown => {
                self.move_by(10);
                Action::Ignore
            }
            KeyCode::Home => {
                self.select_first();
                Action::Ignore
            }
            KeyCode::End => {
                self.select_last();
                Action::Ignore
            }
            KeyCode::Enter => match self.selected(state) {
                Some(server) => Action::Connect(server.clone()),
                None => Action::None,
            },
            KeyCode::Char('d') => Action::Disconnect,
            KeyCode::Char('r') => Action::FetchServers,
            KeyCode::Char('p') => Action::PingAll,
            KeyCode::Char('f') => Self::connect_fastest(state),
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                Action::Ignore
            }
            KeyCode::Char('/') => {
                self.filtering = true;
                self.filter.clear();
                Action::Ignore
            }
            KeyCode::Char('L') => Action::Logout,
            KeyCode::Esc | KeyCode::Char('q') => Action::Quit,
            _ => Action::None,
        }
    }

    fn handle_search_key(&mut self, event: KeyEvent, state: &Store) -> Action {
        match event.code {
            KeyCode::Esc => {
                self.filtering = false;
                self.filter.clear();
                Action::Ignore
            }
            KeyCode::Enter => {
                self.filtering = false;
                match self.selected(state) {
                    Some(server) => Action::Connect(server.clone()),
                    None => Action::Ignore,
                }
            }
            KeyCode::Up => {
                self.move_by(-1);
                Action::Ignore
            }
            KeyCode::Down => {
                self.move_by(1);
                Action::Ignore
            }
            KeyCode::Backspace => {
                self.filter.pop();
                Action::Ignore
            }
            KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.clear();
                Action::Ignore
            }
            KeyCode::Char(c)
                if !event.modifiers.contains(KeyModifiers::CONTROL)
                    && !event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.filter.push(c);
                Action::Ignore
            }
            _ => Action::None,
        }
    }

    pub(super) fn on_action(&mut self, state: &Store, action: &Action) {
        if matches!(action, Action::Latency { .. })
            && state.busy.is_none()
            && state.status_msg.as_deref() == Some("ping complete")
        {
            self.sort = SortMode::Latency;
        }
    }
}
