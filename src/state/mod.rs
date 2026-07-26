mod actions;
mod auth;
mod login;
mod tick;
mod vpn;

use std::collections::HashMap;
use std::time::Instant;

use tokio::sync::mpsc::UnboundedSender;

use crate::api::proton::{self, ProtonTokens};
use crate::api::surfshark;
use crate::api::{Provider, Server, Session};
use crate::utils::{self, conf_path, generate_keypair, Storage, WgStatus};

pub use actions::Action;
pub use login::CodeLogin;

pub struct Store {
    pub should_quit: bool,
    pub is_loading: bool,
    pub busy: Option<String>,
    pub error: Option<String>,
    pub status_msg: Option<String>,
    pub code_login: Option<CodeLogin>,
    /// Proton tokens awaiting a TOTP code.
    pub pending_2fa: Option<ProtonTokens>,
    pub session: Option<Session>,
    pub servers: Vec<Server>,
    /// Bumped when the server list is replaced so the UI can invalidate its index cache.
    pub servers_epoch: u64,
    pub latencies: HashMap<String, Option<u32>>,
    pub connected: Option<String>,
    pub wg_status: Option<WgStatus>,
    pings_pending: usize,
    pings_total: usize,
    storage: Storage,
    last_token_check: Instant,
    last_status_poll: Instant,
    renew_in_flight: bool,
}

impl Store {
    pub fn new() -> Self {
        let mut storage = Storage::load();
        // Ensure provider keys exist before building a live session from disk.
        match storage.provider {
            Provider::Surfshark => {
                if storage.data.token.is_some()
                    && (storage.data.public_key.is_none() || storage.data.private_key.is_none())
                {
                    let keys = generate_keypair();
                    storage.data.private_key = Some(keys.private);
                    storage.data.public_key = Some(keys.public);
                    let _ = storage.save();
                }
            }
            Provider::Proton => {
                if storage.data.token.is_some() && storage.data.ed25519_seed.is_none() {
                    let keys = proton::keys::generate();
                    storage.data.ed25519_seed = Some(keys.seed_b64);
                    let _ = storage.save();
                }
            }
        }

        let session = Self::session_from_storage(&storage);

        let (connected, wg_status) = match utils::wg::status(&conf_path()) {
            Some(status) => {
                let name = storage
                    .data
                    .connected
                    .clone()
                    .unwrap_or_else(|| status.endpoint.clone());
                (Some(name), Some(status))
            }
            None => (None, None),
        };

        let servers = if session.is_some() {
            storage.cached_servers()
        } else {
            Vec::new()
        };

        Self {
            should_quit: false,
            is_loading: false,
            busy: None,
            error: None,
            status_msg: None,
            code_login: None,
            pending_2fa: None,
            session,
            servers,
            servers_epoch: 0,
            latencies: HashMap::new(),
            connected,
            wg_status,
            pings_pending: 0,
            pings_total: 0,
            storage,
            last_token_check: Instant::now(),
            last_status_poll: Instant::now(),
            renew_in_flight: false,
        }
    }

    pub fn provider(&self) -> Provider {
        self.storage.provider
    }

    pub fn email(&self) -> &str {
        self.storage.data.email.as_deref().unwrap_or("")
    }

    pub fn is_busy(&self) -> bool {
        self.is_loading || self.busy.is_some()
    }

    pub(crate) fn save_storage(&mut self) {
        if let Err(e) = self.storage.save() {
            self.error = Some(format!("failed to save session: {e}"));
        }
    }

    fn session_from_storage(storage: &Storage) -> Option<Session> {
        let token = storage.data.token.clone()?;
        Some(Self::make_session(
            storage.provider,
            token,
            storage.data.renew_token.clone(),
            storage.data.uid.clone(),
            storage,
        ))
    }

    fn make_session(
        provider: Provider,
        token: String,
        renew_token: Option<String>,
        uid: Option<String>,
        storage: &Storage,
    ) -> Session {
        match provider {
            Provider::Surfshark => {
                let pub_key = storage.data.public_key.clone().unwrap_or_default();
                Session::Surfshark {
                    auth: surfshark::AuthSession::new(token, renew_token),
                    pub_key,
                }
            }
            Provider::Proton => {
                let keys = storage
                    .data
                    .ed25519_seed
                    .as_deref()
                    .and_then(proton::keys::from_seed_b64)
                    .expect("Proton seed ensured before session build");
                let tokens = ProtonTokens {
                    access_token: token,
                    refresh_token: renew_token.unwrap_or_default(),
                    uid: uid.unwrap_or_default(),
                };
                Session::Proton(proton::ProtonSession::new(tokens, keys))
            }
        }
    }

    /// Ensure Surfshark X25519 keys exist; returns the public key.
    pub(crate) fn ensure_surfshark_keys(&mut self) -> String {
        if self.storage.data.public_key.is_none() || self.storage.data.private_key.is_none() {
            let keys = generate_keypair();
            self.storage.data.private_key = Some(keys.private);
            self.storage.data.public_key = Some(keys.public);
            self.save_storage();
        }
        self.storage.data.public_key.clone().unwrap()
    }

    /// Ensure Proton Ed25519 seed exists; returns derived key material.
    pub(crate) fn ensure_proton_keys(&mut self) -> proton::keys::ProtonKeys {
        if let Some(keys) = self
            .storage
            .data
            .ed25519_seed
            .as_deref()
            .and_then(proton::keys::from_seed_b64)
        {
            return keys;
        }
        let keys = proton::keys::generate();
        self.storage.data.ed25519_seed = Some(keys.seed_b64.clone());
        self.save_storage();
        keys
    }

    pub(crate) fn build_session(
        &mut self,
        token: String,
        renew_token: Option<String>,
        uid: Option<String>,
    ) -> Session {
        match self.storage.provider {
            Provider::Surfshark => {
                let pub_key = self.ensure_surfshark_keys();
                Session::Surfshark {
                    auth: surfshark::AuthSession::new(token, renew_token),
                    pub_key,
                }
            }
            Provider::Proton => {
                let keys = self.ensure_proton_keys();
                let tokens = ProtonTokens {
                    access_token: token,
                    refresh_token: renew_token.unwrap_or_default(),
                    uid: uid.unwrap_or_default(),
                };
                Session::Proton(proton::ProtonSession::new(tokens, keys))
            }
        }
    }

    pub(crate) fn wg_private_key(&self) -> Option<String> {
        match self.storage.provider {
            Provider::Surfshark => self.storage.data.private_key.clone(),
            Provider::Proton => self
                .storage
                .data
                .ed25519_seed
                .as_deref()
                .and_then(proton::keys::from_seed_b64)
                .map(|k| k.wg_private_key),
        }
    }

    pub(crate) fn clear_auth(&mut self, msg: String) {
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
        self.renew_in_flight = false;
        self.is_loading = false;
        self.busy = None;
        self.error = Some(msg);
    }

    /// Switch provider and restore that provider's saved session (if any).
    pub(crate) fn switch_provider(&mut self) {
        if self.is_busy() {
            return;
        }
        let next = self.storage.provider.next();
        if let Err(e) = self.storage.switch_provider(next) {
            self.error = Some(format!("failed to switch provider: {e}"));
            return;
        }
        self.code_login = None;
        self.pending_2fa = None;
        self.servers.clear();
        self.servers_epoch = self.servers_epoch.wrapping_add(1);
        self.latencies.clear();
        self.renew_in_flight = false;
        self.is_loading = false;
        self.busy = None;
        self.error = None;
        self.status_msg = Some(format!("switched to {}", next.label()));
        self.session = Self::session_from_storage(&self.storage);
        if self.session.is_some() {
            self.servers = self.storage.cached_servers();
            self.servers_epoch = self.servers_epoch.wrapping_add(1);
        }
        // Keep showing the live tunnel regardless of which provider owns it.
        if self.connected.is_none()
            && let Some(status) = utils::wg::status(&conf_path())
        {
            self.connected = Some(
                self.storage
                    .data
                    .connected
                    .clone()
                    .unwrap_or_else(|| status.endpoint.clone()),
            );
            self.wg_status = Some(status);
        }
    }

    pub fn handle_action(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Ignore | Action::None => {}

            Action::SwitchProvider => {
                self.switch_provider();
                // Prefer the per-provider disk cache; fetch only if empty.
                if self.session.is_some() && self.servers.is_empty() {
                    let _ = action_tx.send(Action::FetchServers);
                }
            }

            Action::Login { .. }
            | Action::Submit2fa { .. }
            | Action::Cancel2fa
            | Action::TwoFactorRequired { .. }
            | Action::SetQrCode
            | Action::CancelCodeLogin
            | Action::CodeLoginReady { .. }
            | Action::LoggedIn { .. }
            | Action::SessionUpdated { .. }
            | Action::AuthExpired(_)
            | Action::RenewFinished
            | Action::Logout => self.handle_auth(action, action_tx),

            Action::FetchServers
            | Action::ServersLoaded(_)
            | Action::PingAll
            | Action::Latency { .. }
            | Action::Connect(_)
            | Action::Disconnect
            | Action::Connected(_)
            | Action::Disconnected => self.handle_vpn(action, action_tx),

            Action::Tick => self.handle_tick(action_tx),
            Action::Error(msg) => {
                self.is_loading = false;
                self.busy = None;
                self.error = Some(msg);
            }
        }
    }
}
