use serde::Deserialize;

use super::curl::{self, Header};
use super::error::{bearer, is_ok, parse, status_error, transport, Result};
use super::SURFSHARK_BASE_URL;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SurfsharkServer {
    connection_name: String,
    #[serde(default)]
    pub_key: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    load: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub name: String,
    pub country: String,
    pub location: String,
    pub load: u32,
    pub wg_public_key: String,
    pub endpoint_host: String,
}

impl Server {
    pub fn display_name(&self) -> String {
        if self.location.is_empty() {
            self.name.clone()
        } else {
            format!("{}, {}", self.location, self.country)
        }
    }
}

pub fn fetch_servers(token: &str) -> Result<Vec<Server>> {
    let resp = curl::get(
        &format!("{SURFSHARK_BASE_URL}/v4/server/clusters"),
        &[Header("Authorization", &bearer(token))],
    )
    .map_err(|e| transport("server list", e))?;
    if !is_ok(resp.status) {
        return Err(status_error("server list", &resp));
    }
    let raw: Vec<SurfsharkServer> = parse("server list", resp)?;
    Ok(raw
        .into_iter()
        .map(|s| Server {
            name: s.connection_name.clone(),
            country: s.country,
            location: s.location,
            load: s.load,
            wg_public_key: s.pub_key,
            endpoint_host: s.connection_name,
        })
        .collect())
}
