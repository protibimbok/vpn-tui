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
    /// Place label: `Country, Location` (falls back when either is empty).
    pub fn display_name(&self) -> String {
        match (self.country.is_empty(), self.location.is_empty()) {
            (false, false) => format!("{}, {}", self.country, self.location),
            (false, true) => self.country.clone(),
            (true, false) => self.location.clone(),
            (true, true) => self.name.clone(),
        }
    }

    /// List / banner label: `Country, Location (unique-id)`.
    pub fn connected_label(&self) -> String {
        let place = self.display_name();
        if self.name.is_empty() || place == self.name {
            place
        } else {
            format!("{place} ({})", self.name)
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
        .map(|s| {
            const SUFFIX: &str = ".prod.surfshark.com";
            let name = s
                .connection_name
                .strip_suffix(SUFFIX)
                .unwrap_or(&s.connection_name)
                .to_string();
            Server {
                name,
                country: s.country,
                location: s.location,
                load: s.load,
                wg_public_key: s.pub_key,
                endpoint_host: s.connection_name,
            }
        })
        .collect())
}
