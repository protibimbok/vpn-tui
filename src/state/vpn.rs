use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;

use crate::api::{ApiError, Server};
use crate::utils::{self, conf_path, ping_ms};

use super::{Action, Store};

const PING_WORKERS: usize = 16;

impl Store {
    pub(super) fn handle_vpn(&mut self, action: Action, action_tx: &UnboundedSender<Action>) {
        match action {
            Action::FetchServers => {
                if self.busy.is_some() || self.session.is_none() {
                    return;
                }
                // Ensure keys exist and are reflected in the session before bootstrap.
                match self.storage.provider {
                    crate::api::Provider::Surfshark => {
                        let pub_key = self.ensure_surfshark_keys();
                        if let Some(crate::api::Session::Surfshark {
                            pub_key: ref mut pk,
                            ..
                        }) = self.session
                        {
                            *pk = pub_key;
                        }
                    }
                    crate::api::Provider::Proton => {
                        let keys = self.ensure_proton_keys();
                        if let Some(crate::api::Session::Proton(ref mut p)) = self.session {
                            // Rebuild so the session holds the persisted seed.
                            let tokens = p.tokens().clone();
                            *p = crate::api::proton::ProtonSession::new(tokens, keys);
                        }
                    }
                }
                let mut session = self.session.clone().unwrap();
                self.busy = Some(if self.servers.is_empty() {
                    "Loading servers…".into()
                } else {
                    "Refreshing servers…".into()
                });
                self.error = None;
                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = match session.bootstrap() {
                        Ok(servers) => {
                            let snap = session.snapshot();
                            let _ = tx.send(Action::SessionUpdated {
                                token: snap.token,
                                renew_token: snap.renew_token,
                                uid: snap.uid,
                            });
                            tx.send(Action::ServersLoaded(servers))
                        }
                        Err(ApiError::Unauthorized(msg)) => tx.send(Action::AuthExpired(msg)),
                        Err(e) => tx.send(Action::Error(e.to_string())),
                    };
                });
            }
            Action::ServersLoaded(mut servers) => {
                servers.retain(|s| !s.wg_public_key.is_empty());
                let n = servers.len();
                self.storage.set_servers_cache(&servers);
                self.save_storage();
                self.servers = servers;
                self.servers_epoch = self.servers_epoch.wrapping_add(1);
                self.busy = None;
                self.status_msg = Some(format!("{n} servers loaded"));
            }
            Action::PingAll => {
                if self.busy.is_some() || self.servers.is_empty() {
                    return;
                }
                self.latencies.clear();
                self.pings_total = self.servers.len();
                self.pings_pending = self.pings_total;
                self.busy = Some(format!("Pinging… 0/{}", self.pings_total));
                self.error = None;
                let queue: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
                    self.servers
                        .iter()
                        .map(|s| s.endpoint_host.clone())
                        .collect(),
                ));
                for _ in 0..PING_WORKERS.min(self.pings_total) {
                    let queue = Arc::clone(&queue);
                    let tx = action_tx.clone();
                    tokio::task::spawn_blocking(move || loop {
                        let host = queue.lock().ok().and_then(|mut q| q.pop());
                        let Some(host) = host else { break };
                        let ms = ping_ms(&host);
                        if tx.send(Action::Latency { host, ms }).is_err() {
                            break;
                        }
                    });
                }
            }
            Action::Latency { host, ms } => {
                self.latencies.insert(host, ms);
                self.pings_pending = self.pings_pending.saturating_sub(1);
                if self.pings_pending == 0 {
                    self.busy = None;
                    self.status_msg = Some("ping complete".into());
                } else {
                    self.busy = Some(format!(
                        "Pinging… {}/{}",
                        self.pings_total - self.pings_pending,
                        self.pings_total
                    ));
                }
            }
            Action::Connect(server) => {
                if self.busy.is_some() {
                    return;
                }
                let Some(private_key) = self.wg_private_key() else {
                    self.error = Some("no WireGuard key yet — refresh servers first".into());
                    return;
                };
                let provider = self.storage.provider;
                let label = server.connected_label();
                let id = server.name.clone();
                let was_connected = self.connected.is_some();
                let conf = conf_path();
                self.busy = Some(format!("Connecting to {label}…"));
                self.error = None;
                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    if was_connected {
                        let _ = utils::wg::down(&conf);
                    }
                    let result = utils::wg::write_conf(&conf, provider, &private_key, &server)
                        .and_then(|_| utils::wg::up(&conf));
                    let _ = match result {
                        Ok(()) => tx.send(Action::Connected(id)),
                        Err(e) => tx.send(Action::Error(format!("connect: {e}"))),
                    };
                });
            }
            Action::Disconnect => {
                if self.busy.is_some() {
                    return;
                }
                if self.connected.is_none() {
                    self.status_msg = Some("not connected".into());
                    return;
                }
                let conf = conf_path();
                self.busy = Some("Disconnecting…".into());
                self.error = None;
                let tx = action_tx.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = match utils::wg::down(&conf) {
                        Ok(()) => tx.send(Action::Disconnected),
                        Err(e) => tx.send(Action::Error(format!("disconnect: {e}"))),
                    };
                });
            }
            Action::Connected(id) => {
                self.connected = Some(id.clone());
                self.storage.data.connected = Some(id.clone());
                self.save_storage();
                self.busy = None;
                self.wg_status = utils::wg::status(&conf_path());
                let label = self
                    .servers
                    .iter()
                    .find(|s| s.name == id)
                    .map(Server::connected_label)
                    .unwrap_or(id);
                self.status_msg = Some(format!("connected to {label}"));
            }
            Action::Disconnected => {
                self.connected = None;
                self.wg_status = None;
                self.storage.data.connected = None;
                self.save_storage();
                self.busy = None;
                self.status_msg = Some("disconnected".into());
            }
            _ => {}
        }
    }
}
