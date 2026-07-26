use serde::Deserialize;

use super::BASE_URL;
use crate::api::curl::{self, Header};
use crate::api::error::{bearer, is_ok, parse, status_error, transport, Result};
use crate::api::servers::Server;

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

pub fn fetch_servers(token: &str) -> Result<Vec<Server>> {
    let resp = curl::get(
        &format!("{BASE_URL}/v4/server/clusters"),
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
