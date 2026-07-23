use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::api::{proton, Provider};

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
                self.pending_2fa = None;
                let tx = action_tx.clone();
                let provider = self.storage.provider;
                tokio::task::spawn_blocking(move || {
                    let _ = match provider {
                        Provider::Surfshark => match crate::api::login(&username, &password) {
                            Ok(tokens) => tx.send(Action::LoggedIn {
                                token: tokens.token,
                                renew_token: tokens.renew_token,
                                uid: None,
                                email: Some(username),
                            }),
                            Err(e) => tx.send(Action::Error(e.to_string())),
                        },
                        Provider::Proton => match proton::login(&username, &password) {
                            Ok(proton::LoginResult::Success(t)) => tx.send(Action::LoggedIn {
                                token: t.access_token,
                                renew_token: Some(t.refresh_token),
                                uid: Some(t.uid),
                                email: Some(username),
                            }),
                            Ok(proton::LoginResult::TwoFactorRequired(t)) => {
                                tx.send(Action::TwoFactorRequired {
                                    tokens: t,
                                    email: Some(username),
                                })
                            }
                            Err(e) => tx.send(Action::Error(e.to_string())),
                        },
                    };
                });
            }
            Action::TwoFactorRequired { tokens, email } => {
                if email.is_some() {
                    self.storage.data.email = email;
                    self.save_storage();
                }
                self.pending_2fa = Some(tokens);
                self.is_loading = false;
                self.error = None;
                self.status_msg = Some("enter your authenticator code".into());
            }
            Action::Cancel2fa => {
                self.pending_2fa = None;
                self.is_loading = false;
                self.error = None;
                self.status_msg = None;
            }
            Action::Submit2fa { code } => {
                if self.is_busy() {
                    return;
                }
                let Some(tokens) = self.pending_2fa.clone() else {
                    return;
                };
                self.is_loading = true;
                self.error = None;
                let email = self.storage.data.email.clone();
                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = match proton::submit_2fa(&tokens, &code) {
                        Ok(()) => tx.send(Action::LoggedIn {
                            token: tokens.access_token,
                            renew_token: Some(tokens.refresh_token),
                            uid: Some(tokens.uid),
                            email,
                        }),
                        Err(e) => tx.send(Action::Error(e.to_string())),
                    };
                });
            }
            Action::SetQrCode => {
                if self.storage.provider != Provider::Surfshark {
                    return;
                }
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
                uid,
                email,
            } => {
                self.code_login = None;
                self.pending_2fa = None;
                self.storage.data.token = Some(token.clone());
                self.storage.data.renew_token = renew_token.clone();
                self.storage.data.uid = uid.clone();
                if email.is_some() {
                    self.storage.data.email = email;
                }
                // Persist provider keys before building the session.
                match self.storage.provider {
                    Provider::Surfshark => {
                        let _ = self.ensure_surfshark_keys();
                    }
                    Provider::Proton => {
                        let _ = self.ensure_proton_keys();
                    }
                }
                self.save_storage();
                self.session = Some(self.build_session(token, renew_token, uid));
                self.is_loading = false;
                self.error = None;
                self.status_msg = None;
                self.last_token_check = Instant::now();
                if self.servers.is_empty() {
                    self.servers = self.storage.cached_servers();
                }
                // Only hit the network when there is nothing cached; `r` refreshes.
                if self.servers.is_empty() {
                    let _ = action_tx.send(Action::FetchServers);
                }
            }
            Action::SessionUpdated {
                token,
                renew_token,
                uid,
            } => {
                self.renew_in_flight = false;
                self.storage.data.token = Some(token.clone());
                self.storage.data.renew_token = renew_token.clone();
                if uid.is_some() {
                    self.storage.data.uid = uid.clone();
                }
                self.save_storage();
                self.session = Some(self.build_session(token, renew_token, uid));
            }
            Action::AuthExpired(msg) => self.clear_auth(msg),
            Action::RenewFinished => {
                self.renew_in_flight = false;
            }
            Action::Logout => {
                self.storage.data.token = None;
                self.storage.data.renew_token = None;
                self.storage.data.uid = None;
                self.save_storage();
                self.session = None;
                self.code_login = None;
                self.pending_2fa = None;
                self.servers.clear();
                self.servers_epoch = self.servers_epoch.wrapping_add(1);
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
