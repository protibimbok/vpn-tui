//! Servers screen: list / search / connect. Layout adapts to terminal size.

mod input;
mod list;
mod render;

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use crate::{Action, Store, ui::Component};

use list::ServerList;

pub struct ServersPage {
    list: ServerList,
}

impl ServersPage {
    /// Open the servers screen. Fetches from the network only when the
    /// in-memory list is empty (no usable disk cache); otherwise the cache
    /// is shown immediately. Press `r` to force a refresh.
    pub fn new(action_tx: UnboundedSender<Action>, servers_empty: bool) -> Self {
        if servers_empty {
            let _ = action_tx.send(Action::FetchServers);
        }
        Self {
            list: ServerList::new(),
        }
    }
}

impl Component for ServersPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &Store) {
        self.list.render(frame, area, state);
    }

    fn handle_input(&mut self, event: KeyEvent, state: &Store) -> Action {
        self.list.handle_key(event, state)
    }

    fn handle_mouse(&mut self, _event: MouseEvent, _state: &Store) -> Action {
        Action::None
    }

    fn update(&mut self, state: &Store, action: &Action) -> Action {
        self.list.on_action(state, action);
        Action::None
    }
}
