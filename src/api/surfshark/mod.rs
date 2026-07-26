//! Surfshark REST client: password/QR login, JWT renewal, WireGuard key
//! registration, and the server cluster list.
//!
//! Unlike Proton's SRP-6a flow, Surfshark uses a plain password POST (or a
//! device login code polled from the web origin) and registers a plain X25519
//! WireGuard public key. Access tokens are JWTs; renew before `exp` via the
//! renew token.

mod auth;
mod servers;
mod session;

pub use auth::{create_login_code, login, poll_login_code, PollResult};
pub use session::AuthSession;

pub const BASE_URL: &str = "https://api.surfshark.com";
pub const WEB_BASE_URL: &str = "https://my.surfshark.com";
pub const LOGIN_CODE_URL: &str = "https://my.surfshark.com/login-code?login-code=";
