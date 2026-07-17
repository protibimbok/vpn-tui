use ratatui::DefaultTerminal;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{Action, Store, UIApp, UIEvent};

pub struct App {
    store: Store,
    action_rx: UnboundedReceiver<Action>,
    action_tx: UnboundedSender<Action>,
    event_rx: UnboundedReceiver<UIEvent>,
    event_tx: UnboundedSender<UIEvent>,
}

impl App {
    pub fn new() -> Self {
        let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            store: Store::new(),
            action_rx,
            action_tx,
            event_rx,
            event_tx,
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        let mut ui_app = UIApp::new(self.action_tx.clone(), self.event_tx.clone());
        let event_loop = ui_app.spawn_event_loop(1000 / 60);

        while !self.store.should_quit {
            tokio::select! {
                Some(event) = self.event_rx.recv() => {
                    ui_app.handle_event(event);
                }
                Some(action) = self.action_rx.recv() => {
                    self.store.handle_action(action, &self.action_tx);
                }
            }
            ui_app.render(terminal, &self.store);
        }
        event_loop.abort();
        Ok(())
    }
}
