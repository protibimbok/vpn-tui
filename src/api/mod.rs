//! Shared VPN API types and provider clients.

mod curl;
mod error;
mod live;
pub mod proton;
mod provider;
mod servers;
pub mod surfshark;

pub use error::ApiError;
pub use live::Session;
pub use provider::Provider;
pub use servers::Server;
