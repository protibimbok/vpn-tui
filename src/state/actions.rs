#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Login {
        username: String,
        password: String,
    },
    Quit,
    Tick,
    SetQrCode,
    /// Drop any in-flight login-code challenge (stops its poller via Drop).
    CancelCodeLogin,
    /// API finished minting a login code.
    CodeLoginReady {
        code: String,
        hash: String,
        ttl_secs: u64,
    },
    /// Password or login-code flow succeeded.
    LoggedIn {
        token: String,
        renew_token: Option<String>,
        email: Option<String>,
    },
    /// Access token was renewed; persist the new pair.
    SessionUpdated {
        token: String,
        renew_token: Option<String>,
    },
    /// Renew rejected the session — re-login required.
    AuthExpired(String),
    /// Background renew finished without updating tokens (transient failure).
    RenewFinished,
    Error(String),
    Ignore,
    None,
}
