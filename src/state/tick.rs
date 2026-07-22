use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::api::ApiError;
use crate::utils::{self, conf_path};

use super::{Action, Store};

const TOKEN_CHECK_INTERVAL: Duration = Duration::from_secs(60);
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(3);

impl Store {
    pub(super) fn handle_tick(&mut self, action_tx: &UnboundedSender<Action>) {
        if let Some(code) = &self.code_login {
            if Instant::now() >= code.expires_at {
                self.code_login = None;
                let _ = action_tx.send(Action::SetQrCode);
            }
        }

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
                        Err(ApiError::Unauthorized(msg)) => tx.send(Action::AuthExpired(msg)),
                        Err(_) => tx.send(Action::RenewFinished),
                    };
                });
            }
        }

        if self.connected.is_some() && self.last_status_poll.elapsed() >= STATUS_POLL_INTERVAL {
            self.last_status_poll = Instant::now();
            self.wg_status = utils::wg::status(&conf_path());
            if self.wg_status.is_none() {
                self.connected = None;
                self.storage.connected = None;
                self.save_storage();
            }
        }
    }
}
