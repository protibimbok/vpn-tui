mod actions;
mod login;

use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

pub use actions::Action;
pub use login::{CodeLogin};


pub struct Store {
    // Runtime
    pub should_quit: bool,
    // UI
    pub is_loading: bool,
    pub error: Option<String>,
    pub code_login: Option<CodeLogin>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            is_loading: false,
            error: None,
            code_login: None,
        }
    }

    pub fn handle_action(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::SetQrCode => {
                // Drop any previous code (cancels its poller via Drop).
                self.code_login = None;
                self.is_loading = true;
                self.error = None;

                let tx = action_tx.clone();
                tokio::spawn(async move {
                    // Simulate network latency for requesting a login code.
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    let _ = tx.send(Action::CodeLoginReady {
                        code: "AB12CD34".to_string(),
                        ttl_secs: 60,
                    });
                });
            }
            Action::CodeLoginReady { code, ttl_secs } => {
                self.code_login = Some(CodeLogin::new(code, Duration::from_secs(ttl_secs)));
                self.is_loading = false;
                self.error = None;
            }
            _ => {}
        }
    }
}
