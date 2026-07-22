mod actions;
mod auth;
mod login;
mod tick;
mod vpn;

use std::collections::HashMap;
use std::time::Instant;

use tokio::sync::mpsc::UnboundedSender;

use crate::api::{AuthSession, Server};
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
    pub session: Option<AuthSession>,
    pub servers: Vec<Server>,
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
        let storage = Storage::load();
        let session = storage
            .token
            .clone()
            .map(|token| AuthSession::new(token, storage.renew_token.clone()));

        let (connected, wg_status) = match utils::wg::status(&conf_path()) {
            Some(status) => {
                let name = storage
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
            session,
            servers,
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

    pub fn email(&self) -> &str {
        self.storage.email.as_deref().unwrap_or("")
    }

    pub fn is_busy(&self) -> bool {
        self.is_loading || self.busy.is_some()
    }

    pub(crate) fn save_storage(&mut self) {
        if let Err(e) = self.storage.save() {
            self.error = Some(format!("failed to save session: {e}"));
        }
    }

    /// Ensure a Surfshark WireGuard keypair exists (needed for token renew).
    pub(crate) fn ensure_public_key(&mut self) -> String {
        if self.storage.public_key.is_none() || self.storage.private_key.is_none() {
            let keys = generate_keypair();
            self.storage.private_key = Some(keys.private);
            self.storage.public_key = Some(keys.public);
            self.save_storage();
        }
        self.storage.public_key.clone().unwrap()
    }

    pub(crate) fn clear_auth(&mut self, msg: String) {
        self.storage.token = None;
        self.storage.renew_token = None;
        self.save_storage();
        self.session = None;
        self.code_login = None;
        self.servers.clear();
        self.latencies.clear();
        self.renew_in_flight = false;
        self.is_loading = false;
        self.busy = None;
        self.error = Some(msg);
    }

    pub fn handle_action(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Ignore | Action::None => {}

            Action::Login { .. }
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
