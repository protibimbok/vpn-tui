#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Login,
    Quit,
    Tick,
    SetQrCode,
    /// Simulated / real API finished minting a login code.
    CodeLoginReady { code: String, ttl_secs: u64 },
    Ignore,
    None,
}
