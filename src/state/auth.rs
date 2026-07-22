use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::api::AuthSession;

use super::{login, Action, CodeLogin, Store};

impl Store {
    pub(super) fn handle_auth(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::Login { username, password } => {
                if self.is_busy() {
                    return;
                }
                self.is_loading = true;
                self.error = None;
                self.status_msg = None;
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
                self.code_login = None;
                self.storage.token = Some(token.clone());
                self.storage.renew_token = renew_token.clone();
                if email.is_some() {
                    self.storage.email = email;
                }
                self.save_storage();
                self.session = Some(AuthSession::new(token, renew_token));
                self.is_loading = false;
                self.error = None;
                self.status_msg = None;
                self.last_token_check = Instant::now();
                if self.servers.is_empty() {
                    self.servers = self.storage.cached_servers();
                }
                let _ = action_tx.send(Action::FetchServers);
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
            Action::AuthExpired(msg) => self.clear_auth(msg),
            Action::RenewFinished => {
                self.renew_in_flight = false;
            }
            Action::Logout => {
                self.storage.token = None;
                self.storage.renew_token = None;
                self.save_storage();
                self.session = None;
                self.code_login = None;
                self.servers.clear();
                self.latencies.clear();
                self.busy = None;
                self.is_loading = false;
                self.error = None;
                self.status_msg = Some("logged out — VPN connection left as is".into());
            }
            _ => {}
        }
    }
}
