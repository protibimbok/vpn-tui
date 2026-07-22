//! Surfshark REST API client.

mod auth;
mod curl;
mod error;
mod servers;
pub mod session;

pub use auth::{create_login_code, login, poll_login_code, PollResult};
pub use error::ApiError;
pub use servers::Server;
pub use session::AuthSession;

pub const SURFSHARK_BASE_URL: &str = "https://api.surfshark.com";
pub const SURFSHARK_WEB_BASE_URL: &str = "https://my.surfshark.com";
pub const SURFSHARK_LOGIN_CODE_URL: &str = "https://my.surfshark.com/login-code?login-code=";
