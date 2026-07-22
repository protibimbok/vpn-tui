//! Auth session with automatic JWT renewal via Surfshark's renew token.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;

use super::auth::renew_token as renew_auth;
use super::error::{ApiError, Result};

/// Renew the access token this long before its JWT `exp` claim.
const RENEW_MARGIN_SECS: u64 = 5 * 60;

#[derive(Clone)]
pub struct AuthSession {
    pub token: String,
    pub renew_token: Option<String>,
}

impl AuthSession {
    pub fn new(token: String, renew_token: Option<String>) -> Self {
        Self { token, renew_token }
    }

    pub fn expiring_soon(&self) -> bool {
        jwt_expiring_soon(&self.token, RENEW_MARGIN_SECS)
    }

    pub fn renew(&mut self, pub_key: &str) -> Result<()> {
        let renew_token = self.renew_token.as_deref().ok_or_else(|| {
            ApiError::Unauthorized("session expired — log in again".into())
        })?;
        let tokens = renew_auth(renew_token, pub_key)?;
        self.token = tokens.token;
        if let Some(rt) = tokens.renew_token {
            self.renew_token = Some(rt);
        }
        Ok(())
    }
}

fn jwt_expiring_soon(token: &str, margin_secs: u64) -> bool {
    let Some(exp) = jwt_exp_unix(token) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    exp <= now.saturating_add(margin_secs)
}

fn jwt_exp_unix(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    #[derive(Deserialize)]
    struct Claims {
        exp: u64,
    }
    serde_json::from_slice::<Claims>(&bytes).ok().map(|c| c.exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_exp_parsed_from_payload() {
        // {"exp":2000000000}
        let token = "aaa.eyJleHAiOjIwMDAwMDAwMDB9.sig";
        assert_eq!(jwt_exp_unix(token), Some(2_000_000_000));
    }
}
