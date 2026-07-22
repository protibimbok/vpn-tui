use serde::Deserialize;

use super::curl::{self, Header};
use super::error::{
    bearer, body_summary, is_ok, parse, status_error, transport, ApiError, Result,
};
use super::{SURFSHARK_BASE_URL, SURFSHARK_WEB_BASE_URL};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokens {
    pub token: String,
    #[serde(default)]
    pub renew_token: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginCode {
    pub code: String,
    pub hash: String,
    #[serde(rename = "expiresAfter")]
    pub expires_after: u64,
}

pub enum PollResult {
    Pending,
    Approved(AuthTokens),
}

pub fn login(username: &str, password: &str) -> Result<AuthTokens> {
    let body = serde_json::json!({ "username": username, "password": password }).to_string();
    let resp = curl::post_json(&format!("{SURFSHARK_BASE_URL}/v1/auth/login"), &[], &body)
        .map_err(|e| transport("login", e))?;
    if resp.status == 401 || resp.status == 403 {
        return Err(ApiError::Unauthorized(format!(
            "login failed (HTTP {}): check credentials (password login does not support 2FA — use a login code)",
            resp.status
        )));
    }
    if !is_ok(resp.status) {
        return Err(status_error("login", &resp));
    }
    parse("login", resp)
}

pub fn create_login_code() -> Result<LoginCode> {
    let resp = curl::post_json(
        &format!("{SURFSHARK_BASE_URL}/v1/account/authorization/create"),
        &[],
        "{}",
    )
    .map_err(|e| transport("login-code request", e))?;
    if !is_ok(resp.status) {
        return Err(status_error("login-code request", &resp));
    }
    parse("login-code request", resp)
}

pub fn poll_login_code(hash: &str) -> Result<PollResult> {
    let body = serde_json::json!({ "hash": hash }).to_string();
    let resp = curl::post_json(
        &format!("{SURFSHARK_WEB_BASE_URL}/auth/login-code"),
        &[
            Header("Origin", SURFSHARK_WEB_BASE_URL),
            Header("Referer", "https://my.surfshark.com/auth/login/code"),
        ],
        &body,
    )
    .map_err(|e| transport("login-code poll", e))?;
    match resp.status {
        404 => Ok(PollResult::Pending),
        s if is_ok(s) => parse("login-code poll", resp).map(PollResult::Approved),
        _ => Err(status_error("login-code poll", &resp)),
    }
}

pub fn register_public_key(token: &str, pub_key: &str) -> Result<()> {
    let body = serde_json::json!({ "pubKey": pub_key }).to_string();
    let resp = curl::post_json(
        &format!("{SURFSHARK_BASE_URL}/v1/account/users/public-keys"),
        &[Header("Authorization", &bearer(token))],
        &body,
    )
    .map_err(|e| transport("key registration", e))?;
    if is_ok(resp.status) || resp.status == 409 {
        Ok(())
    } else {
        Err(status_error("key registration", &resp))
    }
}

pub fn renew_token(renew_token: &str, pub_key: &str) -> Result<AuthTokens> {
    let body = serde_json::json!({ "pubKey": pub_key }).to_string();
    let resp = curl::post_json(
        &format!("{SURFSHARK_BASE_URL}/v1/auth/renew"),
        &[Header("Authorization", &bearer(renew_token))],
        &body,
    )
    .map_err(|e| transport("token renewal", e))?;
    if resp.status == 401 || resp.status == 403 {
        return Err(ApiError::Unauthorized(format!(
            "token renewal failed (HTTP {}): {}",
            resp.status,
            body_summary(&resp.body)
        )));
    }
    if !is_ok(resp.status) {
        return Err(status_error("token renewal", &resp));
    }
    parse("token renewal", resp)
}
