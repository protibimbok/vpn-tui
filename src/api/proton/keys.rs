//! WireGuard key material for Proton, which — unlike Surfshark's plain X25519
//! keypair — derives from an Ed25519 key (confirmed against
//! `python-proton-vpn-api-core`'s `key_mgr.py`):
//!
//! * We generate a 32-byte Ed25519 seed and register its **Ed25519 public key**
//!   with the API (`/vpn/v1/certificate`, `ClientPublicKey` in SPKI PEM form).
//! * The WireGuard private key is `clamp(SHA512(seed)[..32])` — the same X25519
//!   scalar libsodium's `crypto_sign_ed25519_sk_to_curve25519` produces. The
//!   server derives the matching X25519 public key from the Ed25519 public key,
//!   so we never send it.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha512};

/// DER SubjectPublicKeyInfo prefix for an Ed25519 public key (RFC 8410).
const SPKI_ED25519_PREFIX: [u8; 12] = [
    0x30, 0x2A, 0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70, 0x03, 0x21, 0x00,
];

#[derive(Clone)]
pub struct ProtonKeys {
    /// The Ed25519 seed, base64 — persisted so the same identity survives restarts.
    pub seed_b64: String,
    /// WireGuard interface private key (base64 X25519 scalar).
    pub wg_private_key: String,
    /// Ed25519 public key in SPKI PEM form, sent as `ClientPublicKey`.
    pub ed25519_pk_pem: String,
}

fn derive(seed: [u8; 32]) -> ProtonKeys {
    let ed_pub = SigningKey::from_bytes(&seed).verifying_key().to_bytes();

    // x25519_sk = clamp(SHA512(seed)[..32]); the API derives the matching public
    // key from the Ed25519 key, so we never send our own X25519 public key.
    let mut xsk = [0u8; 32];
    xsk.copy_from_slice(&Sha512::digest(seed)[..32]);
    xsk[0] &= 248;
    xsk[31] &= 127;
    xsk[31] |= 64;

    let mut der = Vec::with_capacity(SPKI_ED25519_PREFIX.len() + 32);
    der.extend_from_slice(&SPKI_ED25519_PREFIX);
    der.extend_from_slice(&ed_pub);
    let ed25519_pk_pem = format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        B64.encode(&der)
    );

    ProtonKeys {
        seed_b64: B64.encode(seed),
        wg_private_key: B64.encode(xsk),
        ed25519_pk_pem,
    }
}

/// Rebuild key material from a previously persisted base64 seed.
pub fn from_seed_b64(seed_b64: &str) -> Option<ProtonKeys> {
    let bytes = B64.decode(seed_b64).ok()?;
    let seed: [u8; 32] = bytes.try_into().ok()?;
    Some(derive(seed))
}

/// Generate a fresh Proton key identity.
pub fn generate() -> ProtonKeys {
    use rand_core::{OsRng, RngCore};
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    derive(seed)
}
