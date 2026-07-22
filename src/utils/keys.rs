//! WireGuard key generation (base64 X25519 pair).

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use x25519_dalek::{PublicKey, StaticSecret};

pub struct KeyPair {
    pub private: String,
    pub public: String,
}

pub fn generate_keypair() -> KeyPair {
    let secret = StaticSecret::random_from_rng(rand_core::OsRng);
    let public = PublicKey::from(&secret);
    KeyPair {
        private: B64.encode(secret.to_bytes()),
        public: B64.encode(public.as_bytes()),
    }
}
