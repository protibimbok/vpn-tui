use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

use super::{Component, UIEvent};
use crate::{
    Action, Store,
    ui::{LoginPage, ServersPage},
};

pub struct UIApp {
    component: Box<dyn Component>,
    showing_servers: bool,
    action_tx: UnboundedSender<Action>,
    event_tx: UnboundedSender<UIEvent>,
}

impl UIApp {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        event_tx: UnboundedSender<UIEvent>,
        state: &Store,
    ) -> Self {
        let showing_servers = state.session.is_some();
        let component: Box<dyn Component> = if showing_servers {
            Box::new(ServersPage::new(
                action_tx.clone(),
                state.servers.is_empty(),
            ))
        } else {
            Box::new(LoginPage::new(action_tx.clone(), state.email().to_string()))
        };
        Self {
            component,
            showing_servers,
            action_tx,
            event_tx,
        }
    }

    /// Switch between login and servers when the session appears/disappears.
    pub fn sync_screen(&mut self, state: &Store) {
        let want_servers = state.session.is_some();
        if want_servers == self.showing_servers {
            return;
        }
        self.showing_servers = want_servers;
        self.component = if want_servers {
            Box::new(ServersPage::new(
                self.action_tx.clone(),
                state.servers.is_empty(),
            ))
        } else {
            Box::new(LoginPage::new(
                self.action_tx.clone(),
                state.email().to_string(),
            ))
        };
    }

    pub fn render(&mut self, terminal: &mut DefaultTerminal, state: &Store) {
        terminal
            .draw(|frame| {
                self.component.render(frame, frame.area(), state);
            })
            .unwrap();
    }

    pub fn spawn_event_loop(&mut self, tick_rate: u64) -> JoinHandle<color_eyre::Result<()>> {
        let action_tx = self.action_tx.clone();
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let mut event_stream = EventStream::new();
            let mut tick = tokio::time::interval(Duration::from_millis(tick_rate));
            loop {
                let next_evt = event_stream.next();
                tokio::select! {
                    _ = tick.tick() => {
                        if action_tx.send(Action::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(evt)) = next_evt => {
                        match evt {
                            Event::Key(key) if event_tx.send(UIEvent::Key(key)).is_err() => {
                                break;
                            }
                            Event::Mouse(mouse)
                                if event_tx.send(UIEvent::Mouse(mouse)).is_err() =>
                            {
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        })
    }

    pub fn handle_event(&mut self, evt: UIEvent, state: &Store) {
        match evt {
            UIEvent::Key(key) => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.action_tx.send(Action::Quit).unwrap();
                    return;
                }
                // Global: Alt+Space switches provider (login or servers).
                if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::ALT) {
                    self.action_tx.send(Action::SwitchProvider).unwrap();
                    return;
                }
                let action = self.component.handle_input(key, state);
                if action != Action::None {
                    self.action_tx.send(action).unwrap();
                } else if key.code == KeyCode::Char('q') {
                    self.action_tx.send(Action::Quit).unwrap();
                }
            }
            UIEvent::Mouse(mouse) => {
                let action = self.component.handle_mouse(mouse, state);
                if action != Action::None {
                    self.action_tx.send(action).unwrap();
                }
            }
        }
    }

    pub fn update(&mut self, state: &Store, action: &Action) {
        let follow_up = self.component.update(state, action);
        if follow_up != Action::None {
            let _ = self.action_tx.send(follow_up);
        }
    }
}
