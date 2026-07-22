use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::api::Server;

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

#[derive(Default, Serialize, Deserialize)]
pub struct Storage {
    pub email: Option<String>,
    pub token: Option<String>,
    pub renew_token: Option<String>,
    pub private_key: Option<String>,
    pub public_key: Option<String>,
    pub connected: Option<String>,
    /// Identity-only server list (no load / ping).
    #[serde(default)]
    pub servers: Vec<CachedServer>,
}

pub fn dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vpn")
}

fn state_path() -> PathBuf {
    dir().join("state.json")
}

pub fn conf_path() -> PathBuf {
    dir().join(format!("{}.conf", crate::utils::IFACE))
}

impl Storage {
    pub fn load() -> Self {
        fs::read_to_string(state_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let path = state_path();
        fs::write(&path, serde_json::to_string_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn cached_servers(&self) -> Vec<Server> {
        self.servers.iter().cloned().map(Server::from).collect()
    }

    pub fn set_servers_cache(&mut self, servers: &[Server]) {
        self.servers = servers.iter().map(CachedServer::from).collect();
    }
}
