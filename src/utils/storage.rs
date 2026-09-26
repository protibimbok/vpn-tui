//! Persisted state: prefs (last provider) + per-provider session files.
//!
//! ```text
//! ~/.config/vpn/
//!   prefs.json       # last selected provider
//!   surfshark.json   # Surfshark session + WG keys + server cache
//!   proton.json      # Proton session + Ed25519 seed + server cache
//!   vpn.conf         # active WireGuard conf
//! ```

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::api::{Provider, Server};

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedServer {
    pub name: String,
    pub country: String,
    pub location: String,
    pub load: u32,
    pub wg_public_key: String,
    pub endpoint_host: String,
}

impl From<&Server> for CachedServer {
    fn from(s: &Server) -> Self {
        Self {
            name: s.name.clone(),
            country: s.country.clone(),
            location: s.location.clone(),
            load: s.load,
            wg_public_key: s.wg_public_key.clone(),
            endpoint_host: s.endpoint_host.clone(),
        }
    }
}

impl From<CachedServer> for Server {
    fn from(s: CachedServer) -> Self {
        Server {
            name: s.name,
            country: s.country,
            location: s.location,
            load: s.load,
            wg_public_key: s.wg_public_key,
            endpoint_host: s.endpoint_host,
        }
    }
}

/// Last-selected provider only — kept separate from session secrets.
#[derive(Default, Serialize, Deserialize)]
struct Prefs {
    #[serde(default)]
    provider: Provider,
}

const DATA_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default, Serialize, Deserialize)]
pub struct ProviderStorage {
    pub version: String,
    pub email: Option<String>,
    pub token: Option<String>,
    pub renew_token: Option<String>,
    /// Proton session id (`x-pm-uid`); unused by Surfshark.
    pub uid: Option<String>,
    /// Surfshark X25519 WireGuard keypair.
    pub private_key: Option<String>,
    pub public_key: Option<String>,
    /// Proton Ed25519 seed (base64); WG key is derived from it.
    pub ed25519_seed: Option<String>,
    pub connected: Option<String>,
    pub servers: Vec<CachedServer>,
}

pub struct Storage {
    pub provider: Provider,
    pub data: ProviderStorage,
}

pub fn dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vpn")
}

fn prefs_path() -> PathBuf {
    dir().join("prefs.json")
}

fn provider_path(provider: Provider) -> PathBuf {
    dir().join(provider.storage_file())
}

pub fn conf_path() -> PathBuf {
    dir().join(format!("{}.conf", crate::utils::IFACE))
}

fn ensure_dir() -> Result<(), String> {
    let dir = dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_json(path: &std::path::Path, value: &impl Serialize) -> Result<(), String> {
    ensure_dir()?;
    fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(path: &std::path::Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn load_prefs() -> Prefs {
    load_json(&prefs_path())
}

fn save_prefs(prefs: &Prefs) -> Result<(), String> {
    write_json(&prefs_path(), prefs)
}

/// Fields that survive a version change; everything else is rebuilt.
#[derive(Default, Deserialize)]
struct SessionOnly {
    email: Option<String>,
    token: Option<String>,
    renew_token: Option<String>,
    uid: Option<String>,
    private_key: Option<String>,
    public_key: Option<String>,
    ed25519_seed: Option<String>,
    /// Live tunnel id; the tunnel outlives an app upgrade.
    connected: Option<String>,
}

impl ProviderStorage {
    fn new() -> Self {
        Self {
            version: DATA_VERSION.to_string(),
            ..Self::default()
        }
    }

    fn from_session(s: SessionOnly) -> Self {
        Self {
            email: s.email,
            token: s.token,
            renew_token: s.renew_token,
            uid: s.uid,
            private_key: s.private_key,
            public_key: s.public_key,
            ed25519_seed: s.ed25519_seed,
            connected: s.connected,
            ..Self::new()
        }
    }
}

fn parse_current(json: &str) -> Option<ProviderStorage> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    if value.get("version")?.as_str()? != DATA_VERSION {
        return None;
    }
    serde_json::from_value(value).ok()
}

fn load_provider(provider: Provider) -> ProviderStorage {
    let path = provider_path(provider);
    let Some(json) = read_provider(&path) else {
        return ProviderStorage::new();
    };
    if let Some(data) = parse_current(&json) {
        return data;
    }
    // Written by another version: keep the session, drop the rest.
    let session = serde_json::from_str(&json).unwrap_or_default();
    let data = ProviderStorage::from_session(session);
    let _ = write_json(&path, &data);
    data
}

fn read_provider(path: &std::path::Path) -> Option<String> {
    if path.exists() {
        return fs::read_to_string(path).ok();
    }
    None
}

impl Storage {
    pub fn load() -> Self {
        let prefs = load_prefs();
        let data = load_provider(prefs.provider);
        Self {
            provider: prefs.provider,
            data,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        write_json(&provider_path(self.provider), &self.data)
    }

    pub fn switch_provider(&mut self, next: Provider) -> Result<(), String> {
        self.save()?;
        self.provider = next;
        save_prefs(&Prefs { provider: next })?;
        self.data = load_provider(next);
        Ok(())
    }

    pub fn cached_servers(&self) -> Vec<Server> {
        self.data
            .servers
            .iter()
            .cloned()
            .map(Server::from)
            .collect()
    }

    pub fn set_servers_cache(&mut self, servers: &[Server]) {
        self.data.servers = servers.iter().map(CachedServer::from).collect();
    }
}
