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
    pub wg_public_key: String,
    pub endpoint_host: String,
}

impl From<&Server> for CachedServer {
    fn from(s: &Server) -> Self {
        Self {
            name: s.name.clone(),
            country: s.country.clone(),
            location: s.location.clone(),
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
            load: 0,
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

/// Per-provider auth + keys + cached servers.
#[derive(Default, Serialize, Deserialize)]
pub struct ProviderStorage {
    pub email: Option<String>,
    pub token: Option<String>,
    pub renew_token: Option<String>,
    /// Proton session id (`x-pm-uid`); unused by Surfshark.
    #[serde(default)]
    pub uid: Option<String>,
    /// Surfshark X25519 WireGuard keypair.
    pub private_key: Option<String>,
    pub public_key: Option<String>,
    /// Proton Ed25519 seed (base64); WG key is derived from it.
    #[serde(default)]
    pub ed25519_seed: Option<String>,
    pub connected: Option<String>,
    #[serde(default)]
    pub servers: Vec<CachedServer>,
}

/// Runtime view: active provider + that provider's storage.
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

/// Legacy single-file path from Surfshark-only builds.
fn legacy_state_path() -> PathBuf {
    dir().join("state.json")
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
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?)
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

fn load_provider(provider: Provider) -> ProviderStorage {
    let path = provider_path(provider);
    if path.exists() {
        return load_json(&path);
    }
    // One-shot migration: old Surfshark-only `state.json` → `surfshark.json`.
    if provider == Provider::Surfshark {
        let legacy = legacy_state_path();
        if legacy.exists() {
            let data: ProviderStorage = load_json(&legacy);
            let _ = write_json(&path, &data);
            let _ = fs::remove_file(&legacy);
            return data;
        }
    }
    ProviderStorage::default()
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

    /// Persist the active provider's session file (not prefs).
    pub fn save(&self) -> Result<(), String> {
        write_json(&provider_path(self.provider), &self.data)
    }

    /// Switch provider: flush current, remember selection, load the other file.
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
