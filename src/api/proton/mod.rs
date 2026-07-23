//! ProtonVPN REST client: SRP-6a login, Ed25519→WireGuard keys, certificate
//! registration, and the logical server list.
//!
//! Unlike Surfshark's plain password POST, Proton uses SRP-6a (see [`srp`]) and
//! registers an Ed25519 key that the API turns into a signed certificate (see
//! [`keys`]). All requests carry an `x-pm-appversion` header the API validates;
//! bump [`APP_VERSION`] if Proton starts rejecting it (HTTP error code 5099).

pub mod keys;
pub mod srp;

use serde::Deserialize;

use super::curl::{self, Header, Response};
use super::error::{ApiError, Result};
use super::servers::Server;

const BASE: &str = "https://vpn-api.proton.me";
/// Proton validates this against a whitelist of known client builds. If auth
/// starts failing with API code 5099 ("client version outdated"), bump this.
const APP_VERSION: &str = "LinuxVPN_4.8.0";
const USER_AGENT: &str = "ProtonVPN/4.8.0 (Linux)";
const WG_CERT_DURATION: &str = "10080 min"; // 7 days

struct AuthCtx<'a> {
    uid: &'a str,
    token: &'a str,
}

fn request(
    path: &str,
    auth: Option<AuthCtx>,
    json: Option<&str>,
) -> std::result::Result<Response, String> {
    let url = format!("{BASE}{path}");
    let bearer = auth.as_ref().map(|a| format!("Bearer {}", a.token));
    let mut headers = vec![
        Header("x-pm-appversion", APP_VERSION),
        Header("Accept", "application/vnd.protonmail.v1+json"),
        Header("User-Agent", USER_AGENT),
    ];
    if let Some(a) = &auth {
        headers.push(Header("x-pm-uid", a.uid));
    }
    if let Some(b) = &bearer {
        headers.push(Header("Authorization", b));
    }
    match json {
        Some(body) => curl::post_json(&url, &headers, body),
        None => curl::get(&url, &headers),
    }
}

fn transport(action: &str, msg: String) -> ApiError {
    ApiError::Other(format!("{action} failed: {msg}"))
}

fn body_msg(body: &str) -> String {
    #[derive(Deserialize)]
    struct E {
        #[serde(rename = "Error")]
        error: Option<String>,
        #[serde(rename = "Code")]
        code: Option<i64>,
    }
    match serde_json::from_str::<E>(body) {
        Ok(e) => match (e.error, e.code) {
            (Some(m), Some(c)) if !m.is_empty() => format!("{m} (code {c})"),
            (Some(m), None) if !m.is_empty() => m,
            _ => trimmed(body),
        },
        _ => trimmed(body),
    }
}

fn trimmed(body: &str) -> String {
    let b = body.trim();
    if b.is_empty() {
        "empty response".into()
    } else {
        b.into()
    }
}

fn status_error(action: &str, resp: &Response) -> ApiError {
    let msg = format!(
        "{action} failed (HTTP {}): {}",
        resp.status,
        body_msg(&resp.body)
    );
    if resp.status == 401 || resp.status == 403 {
        ApiError::Unauthorized(msg)
    } else {
        ApiError::Other(msg)
    }
}

fn is_ok(status: u16) -> bool {
    (200..300).contains(&status)
}

fn parse<T: serde::de::DeserializeOwned>(action: &str, resp: Response) -> Result<T> {
    serde_json::from_str(&resp.body)
        .map_err(|e| ApiError::Other(format!("{action}: unexpected response: {e}")))
}

// ---- auth flow ------------------------------------------------------------

#[derive(Deserialize)]
struct AuthInfo {
    #[serde(rename = "Modulus")]
    modulus: String,
    #[serde(rename = "ServerEphemeral")]
    server_ephemeral: String,
    #[serde(rename = "Version")]
    version: u32,
    #[serde(rename = "Salt")]
    salt: String,
    #[serde(rename = "SRPSession")]
    srp_session: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(rename = "UID")]
    uid: String,
    #[serde(rename = "ServerProof")]
    server_proof: String,
    #[serde(rename = "2FA")]
    two_fa: Option<TwoFactorInfo>,
}

#[derive(Deserialize)]
struct TwoFactorInfo {
    #[serde(rename = "Enabled")]
    enabled: u32,
}

/// Session tokens issued by Proton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtonTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub uid: String,
}

/// Outcome of a password login: fully authenticated, or awaiting TOTP.
pub enum LoginResult {
    Success(ProtonTokens),
    TwoFactorRequired(ProtonTokens),
}

/// Run the SRP-6a password login.
pub fn login(email: &str, password: &str) -> Result<LoginResult> {
    let info: AuthInfo = {
        let body = serde_json::json!({ "Username": email }).to_string();
        let resp = request("/auth/v4/info", None, Some(&body))
            .map_err(|e| transport("login (info)", e))?;
        if !is_ok(resp.status) {
            return Err(status_error("login (info)", &resp));
        }
        parse("login (info)", resp)?
    };

    let proofs = srp::compute_proofs(
        info.version,
        password.as_bytes(),
        &info.salt,
        &info.modulus,
        &info.server_ephemeral,
    )
    .map_err(|e| ApiError::Other(format!("SRP: {e}")))?;

    let body = serde_json::json!({
        "Username": email,
        "ClientEphemeral": proofs.client_ephemeral,
        "ClientProof": proofs.client_proof,
        "SRPSession": info.srp_session,
    })
    .to_string();
    let resp =
        request("/auth/v4", None, Some(&body)).map_err(|e| transport("login", e))?;
    if resp.status == 401 || resp.status == 403 || resp.status == 422 {
        return Err(ApiError::Unauthorized(format!(
            "login failed (HTTP {}): {}",
            resp.status,
            body_msg(&resp.body)
        )));
    }
    if !is_ok(resp.status) {
        return Err(status_error("login", &resp));
    }
    let auth: AuthResponse = parse("login", resp)?;

    let expected = base64_std(&proofs.expected_server_proof);
    if auth.server_proof != expected {
        return Err(ApiError::Other(
            "login failed: server SRP proof did not verify (possible MITM)".into(),
        ));
    }

    let tokens = ProtonTokens {
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        uid: auth.uid,
    };
    // 2FA `Enabled` is a bitmask; bit 0 is TOTP.
    let needs_totp = auth.two_fa.map(|t| t.enabled & 1 != 0).unwrap_or(false);
    if needs_totp {
        Ok(LoginResult::TwoFactorRequired(tokens))
    } else {
        Ok(LoginResult::Success(tokens))
    }
}

/// Submit a TOTP code to elevate a two-factor session to full scope.
pub fn submit_2fa(tokens: &ProtonTokens, code: &str) -> Result<()> {
    let body = serde_json::json!({ "TwoFactorCode": code.trim() }).to_string();
    let resp = request(
        "/auth/v4/2fa",
        Some(AuthCtx {
            uid: &tokens.uid,
            token: &tokens.access_token,
        }),
        Some(&body),
    )
    .map_err(|e| transport("2FA", e))?;
    if resp.status == 401 || resp.status == 403 || resp.status == 422 {
        return Err(ApiError::Unauthorized(format!(
            "two-factor code rejected (HTTP {}): {}",
            resp.status,
            body_msg(&resp.body)
        )));
    }
    if !is_ok(resp.status) {
        return Err(status_error("2FA", &resp));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RefreshResponse {
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(rename = "UID")]
    uid: Option<String>,
}

fn refresh_tokens(tokens: &ProtonTokens) -> Result<ProtonTokens> {
    let body = serde_json::json!({
        "ResponseType": "token",
        "GrantType": "refresh_token",
        "RefreshToken": tokens.refresh_token,
        "RedirectURI": "http://protonmail.ch",
    })
    .to_string();
    let resp = request(
        "/auth/refresh",
        Some(AuthCtx {
            uid: &tokens.uid,
            token: &tokens.access_token,
        }),
        Some(&body),
    )
    .map_err(|e| transport("token refresh", e))?;
    if resp.status == 401 || resp.status == 403 {
        return Err(ApiError::Unauthorized(format!(
            "session expired (HTTP {}): log in again",
            resp.status
        )));
    }
    if !is_ok(resp.status) {
        return Err(status_error("token refresh", &resp));
    }
    let r: RefreshResponse = parse("token refresh", resp)?;
    Ok(ProtonTokens {
        access_token: r.access_token,
        refresh_token: r.refresh_token,
        uid: r.uid.unwrap_or_else(|| tokens.uid.clone()),
    })
}

// ---- certificate + server list -------------------------------------------

fn register_certificate(tokens: &ProtonTokens, ed25519_pk_pem: &str) -> Result<()> {
    let body = serde_json::json!({
        "ClientPublicKey": ed25519_pk_pem,
        "Duration": WG_CERT_DURATION,
    })
    .to_string();
    let resp = request(
        "/vpn/v1/certificate",
        Some(AuthCtx {
            uid: &tokens.uid,
            token: &tokens.access_token,
        }),
        Some(&body),
    )
    .map_err(|e| transport("key registration", e))?;
    if resp.status == 401 || resp.status == 403 {
        return Err(status_error("key registration", &resp));
    }
    if is_ok(resp.status) || resp.status == 409 {
        Ok(())
    } else {
        Err(status_error("key registration", &resp))
    }
}

#[derive(Deserialize)]
struct VpnInfoResponse {
    #[serde(rename = "VPN")]
    vpn: VpnTier,
}

#[derive(Deserialize)]
struct VpnTier {
    #[serde(rename = "MaxTier")]
    max_tier: u32,
}

fn fetch_max_tier(tokens: &ProtonTokens) -> Result<u32> {
    let resp = request(
        "/vpn/v2",
        Some(AuthCtx {
            uid: &tokens.uid,
            token: &tokens.access_token,
        }),
        None,
    )
    .map_err(|e| transport("account info", e))?;
    if !is_ok(resp.status) {
        return Err(status_error("account info", &resp));
    }
    let info: VpnInfoResponse = parse("account info", resp)?;
    Ok(info.vpn.max_tier)
}

#[derive(Deserialize)]
struct LogicalsResponse {
    #[serde(rename = "LogicalServers")]
    logical_servers: Vec<LogicalServer>,
}

#[derive(Deserialize)]
struct LogicalServer {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "ExitCountry", default)]
    exit_country: String,
    #[serde(rename = "City", default)]
    city: Option<String>,
    #[serde(rename = "Tier", default)]
    tier: u32,
    #[serde(rename = "Load", default)]
    load: u32,
    #[serde(rename = "Status", default)]
    status: u32,
    #[serde(rename = "Servers", default)]
    servers: Vec<PhysicalServer>,
}

#[derive(Deserialize)]
struct PhysicalServer {
    #[serde(rename = "EntryIP", default)]
    entry_ip: String,
    #[serde(rename = "X25519PublicKey", default)]
    x25519_public_key: String,
    #[serde(rename = "Status", default)]
    status: u32,
}

fn fetch_logicals(tokens: &ProtonTokens, max_tier: u32) -> Result<Vec<Server>> {
    let resp = request(
        "/vpn/v1/logicals?SecureCoreFilter=all&WithState=true",
        Some(AuthCtx {
            uid: &tokens.uid,
            token: &tokens.access_token,
        }),
        None,
    )
    .map_err(|e| transport("server list", e))?;
    if !is_ok(resp.status) {
        return Err(status_error("server list", &resp));
    }
    let logicals: LogicalsResponse = parse("server list", resp)?;

    let mut servers = Vec::new();
    for lg in logicals.logical_servers {
        if lg.tier > max_tier || lg.status != 1 {
            continue;
        }
        let phys = lg
            .servers
            .into_iter()
            .find(|p| p.status == 1 && !p.x25519_public_key.is_empty() && !p.entry_ip.is_empty());
        let Some(phys) = phys else { continue };
        servers.push(Server {
            name: lg.name,
            country: lg.exit_country,
            location: lg.city.unwrap_or_default(),
            load: lg.load,
            wg_public_key: phys.x25519_public_key,
            endpoint_host: phys.entry_ip,
        });
    }
    Ok(servers)
}

// ---- session --------------------------------------------------------------

/// Live Proton session: opaque tokens + derived WireGuard identity.
/// Renewal is reactive (refresh on 401); tokens have no inspectable expiry.
#[derive(Clone)]
pub struct ProtonSession {
    tokens: ProtonTokens,
    keys: keys::ProtonKeys,
}

impl ProtonSession {
    pub fn new(tokens: ProtonTokens, keys: keys::ProtonKeys) -> Self {
        Self { tokens, keys }
    }

    pub fn tokens(&self) -> &ProtonTokens {
        &self.tokens
    }

    fn refresh(&mut self) -> Result<()> {
        self.tokens = refresh_tokens(&self.tokens)?;
        Ok(())
    }

    pub fn renew(&mut self) -> Result<()> {
        self.refresh()
    }

    fn with_retry<T>(&mut self, op: impl Fn(&ProtonTokens) -> Result<T>) -> Result<T> {
        match op(&self.tokens) {
            Err(ApiError::Unauthorized(_)) => {
                self.refresh()?;
                op(&self.tokens)
            }
            other => other,
        }
    }

    pub fn bootstrap(&mut self) -> Result<Vec<Server>> {
        let pem = self.keys.ed25519_pk_pem.clone();
        self.with_retry(|t| register_certificate(t, &pem))?;
        self.fetch_servers()
    }

    pub fn fetch_servers(&mut self) -> Result<Vec<Server>> {
        let max_tier = self.with_retry(fetch_max_tier)?;
        self.with_retry(|t| fetch_logicals(t, max_tier))
    }
}

fn base64_std(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    STANDARD.encode(bytes)
}
