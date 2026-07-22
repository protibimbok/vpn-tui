mod actions;
mod login;

use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::api::{ApiError, AuthSession};
use crate::utils::{generate_keypair, Storage};

pub use actions::Action;
pub use login::CodeLogin;

const TOKEN_CHECK_INTERVAL: Duration = Duration::from_secs(60);

pub struct Store {
    // Runtime
    pub should_quit: bool,
    // UI
    pub is_loading: bool,
    pub error: Option<String>,
    pub code_login: Option<CodeLogin>,
    pub session: Option<AuthSession>,
    storage: Storage,
    last_token_check: Instant,
    renew_in_flight: bool,
}

impl Store {
    pub fn new() -> Self {
        let storage = Storage::load();
        let session = storage
            .token
            .clone()
            .map(|token| AuthSession::new(token, storage.renew_token.clone()));
        Self {
            should_quit: false,
            is_loading: false,
            error: None,
            code_login: None,
            session,
            storage,
            last_token_check: Instant::now(),
            renew_in_flight: false,
        }
    }

    pub fn email(&self) -> &str {
        self.storage.email.as_deref().unwrap_or("")
    }

    fn save_storage(&mut self) {
        if let Err(e) = self.storage.save() {
            self.error = Some(format!("failed to save session: {e}"));
        }
    }

    /// Ensure a Surfshark WireGuard keypair exists (needed for token renew).
    fn ensure_public_key(&mut self) -> String {
        if self.storage.public_key.is_none() || self.storage.private_key.is_none() {
            let keys = generate_keypair();
            self.storage.private_key = Some(keys.private);
            self.storage.public_key = Some(keys.public);
            self.save_storage();
        }
        self.storage.public_key.clone().unwrap()
    }

    fn clear_auth(&mut self, msg: String) {
        self.storage.token = None;
        self.storage.renew_token = None;
        self.save_storage();
        self.session = None;
        self.code_login = None;
        self.renew_in_flight = false;
        self.is_loading = false;
        self.error = Some(msg);
    }

    pub fn handle_action(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::Login { username, password } => {
                if self.is_loading {
                    return;
                }
                self.is_loading = true;
                self.error = None;
                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = match crate::api::login(&username, &password) {
                        Ok(tokens) => tx.send(Action::LoggedIn {
                            token: tokens.token,
                            renew_token: tokens.renew_token,
                            email: Some(username),
                        }),
                        Err(e) => tx.send(Action::Error(e.to_string())),
                    };
                });
            }
            Action::SetQrCode => {
                // Drop any previous code (cancels its poller via Drop).
                self.code_login = None;
                self.is_loading = true;
                self.error = None;

                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || match crate::api::create_login_code() {
                    Ok(lc) => {
                        let _ = tx.send(Action::CodeLoginReady {
                            code: lc.code,
                            hash: lc.hash,
                            ttl_secs: lc.expires_after,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(Action::Error(e.to_string()));
                    }
                });
            }
            Action::CancelCodeLogin => {
                self.code_login = None;
                self.is_loading = false;
                self.error = None;
            }
            Action::CodeLoginReady {
                code,
                hash,
                ttl_secs,
            } => {
                let (challenge, cancel) = CodeLogin::new(code, Duration::from_secs(ttl_secs));
                self.code_login = Some(challenge);
                self.is_loading = false;
                self.error = None;

                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    login::poll_login_code(&tx, &hash, &cancel);
                });
            }
            Action::LoggedIn {
                token,
                renew_token,
                email,
            } => {
                self.code_login = None; // stops its poller via Drop
                self.storage.token = Some(token.clone());
                self.storage.renew_token = renew_token.clone();
                if email.is_some() {
                    self.storage.email = email;
                }
                self.save_storage();
                self.session = Some(AuthSession::new(token, renew_token));
                self.is_loading = false;
                self.error = None;
                self.last_token_check = Instant::now();
            }
            Action::SessionUpdated { token, renew_token } => {
                self.renew_in_flight = false;
                self.storage.token = Some(token.clone());
                self.storage.renew_token = renew_token.clone();
                self.save_storage();
                if let Some(session) = &mut self.session {
                    session.token = token;
                    session.renew_token = renew_token;
                } else {
                    self.session = Some(AuthSession::new(token, renew_token));
                }
            }
            Action::AuthExpired(msg) => {
                self.clear_auth(msg);
            }
            Action::RenewFinished => {
                self.renew_in_flight = false;
            }
            Action::Error(msg) => {
                self.is_loading = false;
                self.error = Some(msg);
            }
            Action::Tick => {
                // Auto-refresh an expired login code so the screen stays usable.
                if let Some(code) = &self.code_login {
                    if Instant::now() >= code.expires_at {
                        // Clear first so subsequent ticks don't re-request.
                        self.code_login = None;
                        let _ = action_tx.send(Action::SetQrCode);
                    }
                }

                // Proactively renew the access token before JWT exp.
                if self.session.is_some()
                    && self.storage.renew_token.is_some()
                    && !self.renew_in_flight
                    && self.last_token_check.elapsed() >= TOKEN_CHECK_INTERVAL
                {
                    self.last_token_check = Instant::now();
                    let expiring = self
                        .session
                        .as_ref()
                        .map(|s| s.expiring_soon())
                        .unwrap_or(false);
                    if expiring {
                        let pub_key = self.ensure_public_key();
                        let mut session = self.session.clone().unwrap();
                        self.renew_in_flight = true;
                        let tx = action_tx.clone();
                        tokio::task::spawn_blocking(move || {
                            let _ = match session.renew(&pub_key) {
                                Ok(()) => tx.send(Action::SessionUpdated {
                                    token: session.token,
                                    renew_token: session.renew_token,
                                }),
                                Err(ApiError::Unauthorized(msg)) => {
                                    tx.send(Action::AuthExpired(msg))
                                }
                                Err(_) => tx.send(Action::RenewFinished),
                            };
                        });
                    }
                }
            }
            Action::Ignore | Action::None => {}
        }
    }
}
