//! Provider-agnostic live session. Surfshark uses JWT + renew token; Proton
//! uses opaque access/refresh tokens + an Ed25519-derived WireGuard identity.

use super::error::Result;
use super::proton::ProtonSession;
use super::servers::Server;
use super::session::AuthSession;

/// Persistable token snapshot common to both providers (`uid` is Proton-only).
pub struct Snapshot {
    pub token: String,
    pub renew_token: Option<String>,
    pub uid: Option<String>,
}

/// Live session for the active provider. Keeps worker threads provider-agnostic.
#[derive(Clone)]
pub enum Session {
    Surfshark {
        auth: AuthSession,
        pub_key: String,
    },
    Proton(ProtonSession),
}

impl Session {
    pub fn bootstrap(&mut self) -> Result<Vec<Server>> {
        match self {
            Session::Surfshark { auth, pub_key } => auth.bootstrap(pub_key),
            Session::Proton(p) => p.bootstrap(),
        }
    }

    /// Whether the access token should be renewed proactively. Surfshark
    /// inspects its JWT `exp`; Proton tokens are opaque and refresh on 401.
    pub fn expiring_soon(&self) -> bool {
        match self {
            Session::Surfshark { auth, .. } => auth.expiring_soon(),
            Session::Proton(_) => false,
        }
    }

    pub fn renew(&mut self) -> Result<()> {
        match self {
            Session::Surfshark { auth, pub_key } => auth.renew(pub_key),
            Session::Proton(p) => p.renew(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        match self {
            Session::Surfshark { auth, .. } => Snapshot {
                token: auth.token.clone(),
                renew_token: auth.renew_token.clone(),
                uid: None,
            },
            Session::Proton(p) => {
                let t = p.tokens();
                Snapshot {
                    token: t.access_token.clone(),
                    renew_token: Some(t.refresh_token.clone()),
                    uid: Some(t.uid.clone()),
                }
            }
        }
    }
}
