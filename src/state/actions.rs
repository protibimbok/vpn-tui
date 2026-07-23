use crate::api::Server;
use crate::api::proton::ProtonTokens;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Login {
        username: String,
        password: String,
    },
    /// Submit a Proton TOTP code for a pending two-factor login.
    Submit2fa {
        code: String,
    },
    Cancel2fa,
    TwoFactorRequired {
        tokens: ProtonTokens,
        email: Option<String>,
    },
    /// Cycle Surfshark ⇄ Proton and load that provider's saved session.
    SwitchProvider,
    Quit,
    Tick,
    SetQrCode,
    CancelCodeLogin,
    CodeLoginReady {
        code: String,
        hash: String,
        ttl_secs: u64,
    },
    LoggedIn {
        token: String,
        renew_token: Option<String>,
        uid: Option<String>,
        email: Option<String>,
    },
    SessionUpdated {
        token: String,
        renew_token: Option<String>,
        uid: Option<String>,
    },
    AuthExpired(String),
    RenewFinished,
    FetchServers,
    ServersLoaded(Vec<Server>),
    PingAll,
    Latency {
        host: String,
        ms: Option<u32>,
    },
    Connect(Server),
    Disconnect,
    Connected(String),
    Disconnected,
    Logout,
    Error(String),
    /// Consumed by UI; do not treat as unhandled (e.g. avoid `q` → quit).
    Ignore,
    None,
}
