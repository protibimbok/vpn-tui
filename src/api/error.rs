use std::fmt;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::curl::Response;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    Other(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Unauthorized(msg) | ApiError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub(crate) type Result<T> = std::result::Result<T, ApiError>;

pub(crate) fn transport(action: &str, msg: String) -> ApiError {
    ApiError::Other(format!("{action} failed: {msg}"))
}

pub(crate) fn status_error(action: &str, resp: &Response) -> ApiError {
    let msg = format!(
        "{action} failed (HTTP {}): {}",
        resp.status,
        body_summary(&resp.body)
    );
    if resp.status == 401 || resp.status == 403 {
        ApiError::Unauthorized(msg)
    } else {
        ApiError::Other(msg)
    }
}

pub(crate) fn body_summary(body: &str) -> String {
    #[derive(Deserialize)]
    struct ErrorBody {
        message: String,
    }
    match serde_json::from_str::<ErrorBody>(body) {
        Ok(e) if !e.message.is_empty() => e.message,
        _ => {
            let body = body.trim();
            if body.is_empty() {
                "empty response".into()
            } else {
                body.into()
            }
        }
    }
}

pub(crate) fn parse<T: DeserializeOwned>(action: &str, resp: Response) -> Result<T> {
    serde_json::from_str(&resp.body)
        .map_err(|e| ApiError::Other(format!("{action}: unexpected response: {e}")))
}

pub(crate) fn is_ok(status: u16) -> bool {
    (200..300).contains(&status)
}

pub(crate) fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
