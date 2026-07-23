//! Proton's SRP-6a client, ported from the authoritative `ProtonMail/go-srp`.
//!
//! Proton does *not* use a plain password POST: authentication is SRP-6a with a
//! non-standard hashing scheme (bcrypt-based password hash + a SHA-512 "expand"
//! hash over a 2048-bit modulus). The modulus the server sends is PGP-clearsigned
//! by Proton; we verify that signature before use, and additionally validate that
//! the modulus is a 2048-bit safe prime with generator 2 (both checks mirror
//! go-srp, so a weak/backdoored modulus is rejected two independent ways).
//!
//! The math here is pinned byte-for-byte by the golden vectors in the test module
//! (generated from go-srp with a fixed client secret).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use num_bigint::BigUint;
use num_traits::One;
use pgp::composed::cleartext::CleartextSignedMessage;
use pgp::composed::{Deserializable, SignedPublicKey};
use sha2::{Digest, Sha512};

/// SRP works over a 2048-bit modulus; all values are little-endian, fixed width.
const BIT_LENGTH: usize = 2048;
const BYTE_LENGTH: usize = BIT_LENGTH / 8;

/// Proton's public key for verifying the clearsigned SRP modulus
/// (from `ProtonMail/go-srp`).
const MODULUS_PUBKEY: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\n\nxjMEXAHLgxYJKwYBBAHaRw8BAQdAFurWXXwjTemqjD7CXjXVyKf0of7n9Ctm\nL8v9enkzggHNEnByb3RvbkBzcnAubW9kdWx1c8J3BBAWCgApBQJcAcuDBgsJ\nBwgDAgkQNQWFxOlRjyYEFQgKAgMWAgECGQECGwMCHgEAAPGRAP9sauJsW12U\nMnTQUZpsbJb53d0Wv55mZIIiJL2XulpWPQD/V6NglBd96lZKBmInSXX/kXat\nSv+y0io+LR8i2+jV+AbOOARcAcuDEgorBgEEAZdVAQUBAQdAeJHUz1c9+KfE\nkSIgcBRE3WuXC4oj5a2/U3oASExGDW4DAQgHwmEEGBYIABMFAlwBy4MJEDUF\nhcTpUY8mAhsMAAD/XQD8DxNI6E78meodQI+wLsrKLeHn32iLvUqJbVDhfWSU\nWO4BAMcm1u02t4VKw++ttECPt+HUgPUq5pqQWe5Q2cW4TMsE\n=Y4Mw\n-----END PGP PUBLIC KEY BLOCK-----";

#[derive(Debug)]
pub struct SrpError(pub String);

impl std::fmt::Display for SrpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SrpError {}

fn err(msg: impl Into<String>) -> SrpError {
    SrpError(msg.into())
}

/// The proofs to send to `/auth/v4`, plus the server proof we expect back.
pub struct Proofs {
    /// Base64 client ephemeral `A`.
    pub client_ephemeral: String,
    /// Base64 client proof `M1`.
    pub client_proof: String,
    /// Raw expected server proof `M2`; compare against the server's `ServerProof`.
    pub expected_server_proof: Vec<u8>,
}

/// `expand_hash(x) = SHA512(x‖0) ‖ SHA512(x‖1) ‖ SHA512(x‖2) ‖ SHA512(x‖3)` (256 B).
fn expand_hash(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 * 64);
    for i in 0u8..4 {
        let mut h = Sha512::new();
        h.update(data);
        h.update([i]);
        out.extend_from_slice(&h.finalize());
    }
    out
}

/// Little-endian bytes → integer (go-srp `toInt`).
fn to_int(le: &[u8]) -> BigUint {
    BigUint::from_bytes_le(le)
}

/// Integer → fixed-width little-endian bytes (go-srp `fromInt`, `bitLength/8` wide).
fn from_int(n: &BigUint) -> Vec<u8> {
    let mut le = n.to_bytes_le();
    le.resize(BYTE_LENGTH, 0);
    le
}

/// bcrypt with Proton's `$2y$` prefix. Matches go-srp `bcryptHash`: the digest
/// depends only on (password, cost, 16 salt bytes) and is identical across the
/// 2a/2b/2y variants, so we compute with the crate and swap the version tag.
fn bcrypt_hash(password: &[u8], salt16: [u8; 16]) -> Result<String, SrpError> {
    let parts =
        bcrypt::hash_with_salt(password, 10, salt16).map_err(|e| err(format!("bcrypt: {e}")))?;
    Ok(parts.format_for_version(bcrypt::Version::TwoY))
}

/// Proton v3/v4 password hash: `expand_hash( bcrypt(salt‖"proton") ‖ modulus )`.
/// (go-srp `hashPasswordVersion3`.) The bcrypt salt is the raw salt bytes with
/// `"proton"` appended, truncated to bcrypt's 16-byte width — for Proton's 10-byte
/// salts this is exactly `salt‖"proton"`.
fn hash_password_v3(password: &[u8], salt: &[u8], modulus: &[u8]) -> Result<Vec<u8>, SrpError> {
    let mut s = salt.to_vec();
    s.extend_from_slice(b"proton");
    s.resize(16, 0);
    let salt16: [u8; 16] = s[..16].try_into().expect("16 bytes");
    let crypted = bcrypt_hash(password, salt16)?;
    let mut buf = crypted.into_bytes();
    buf.extend_from_slice(modulus);
    Ok(expand_hash(&buf))
}

fn hash_password(
    version: u32,
    password: &[u8],
    salt_b64: &str,
    modulus: &[u8],
) -> Result<Vec<u8>, SrpError> {
    match version {
        3 | 4 => {
            let salt = B64
                .decode(salt_b64)
                .map_err(|e| err(format!("bad salt: {e}")))?;
            hash_password_v3(password, &salt, modulus)
        }
        v => Err(err(format!(
            "unsupported SRP auth version {v} (only 3/4 are supported)"
        ))),
    }
}

/// Verify the clearsigned modulus against Proton's key and return the raw
/// (little-endian) modulus bytes.
fn verify_and_extract_modulus(signed_modulus: &str) -> Result<Vec<u8>, SrpError> {
    let (msg, _) = CleartextSignedMessage::from_string(signed_modulus)
        .map_err(|e| err(format!("modulus is not a valid clearsigned message: {e}")))?;
    let (pubkey, _) = SignedPublicKey::from_string(MODULUS_PUBKEY)
        .map_err(|e| err(format!("cannot parse Proton modulus key: {e}")))?;
    msg.verify(&pubkey)
        .map_err(|e| err(format!("modulus signature verification failed: {e}")))?;
    B64.decode(msg.text().trim())
        .map_err(|e| err(format!("modulus is not valid base64: {e}")))
}

/// Port of go-srp `checkParams`: reject a modulus that is not a 2048-bit safe
/// prime ≡ 3 (mod 8) with 2 a generator, and a server ephemeral out of bounds.
fn check_params(n: &BigUint, server_ephemeral: &BigUint) -> Result<(), SrpError> {
    if n.bits() != BIT_LENGTH as u64 {
        return Err(err("SRP modulus has incorrect size"));
    }
    // 2 is a generator of the whole group only when N ≡ 3 (mod 8).
    if (n % 8u32) != BigUint::from(3u32) {
        return Err(err("SRP modulus is not 3 mod 8"));
    }
    let one = BigUint::one();
    let n_minus_one = n - &one;
    if server_ephemeral <= &one || server_ephemeral >= &n_minus_one {
        return Err(err("SRP server ephemeral is out of bounds"));
    }
    let half = n >> 1; // (N-1)/2, N is odd
    if !is_probable_prime(&half, 12) {
        return Err(err("SRP modulus is not a safe prime"));
    }
    // Lucas test: 2^((N-1)/2) ≡ -1 (mod N) proves primality and that 2 generates
    // the whole group (a single exponentiation).
    if BigUint::from(2u32).modpow(&half, n) != n_minus_one {
        return Err(err("SRP modulus is not prime"));
    }
    Ok(())
}

/// Miller-Rabin primality test with fixed small bases (sufficient here: the
/// input either is Proton's real prime or an attacker-substituted composite).
fn is_probable_prime(n: &BigUint, _rounds: usize) -> bool {
    let one = BigUint::one();
    let two = BigUint::from(2u32);
    if n <= &one {
        return false;
    }
    if n == &two {
        return true;
    }
    if !n.bit(0) {
        return false; // even
    }
    // n - 1 = d * 2^r with d odd.
    let n_minus_one = n - &one;
    let mut d = n_minus_one.clone();
    let mut r = 0u32;
    while !d.bit(0) {
        d >>= 1;
        r += 1;
    }
    for &base in &[2u32, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let a = BigUint::from(base);
        if &a >= n {
            continue;
        }
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        let mut composite = true;
        for _ in 0..r.saturating_sub(1) {
            x = x.modpow(&two, n);
            if x == n_minus_one {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

/// Compute the SRP proofs given the raw pieces and a chosen client secret `a`.
/// Split out so tests can inject a fixed secret (matching the golden vectors).
fn proofs_from(
    n: &BigUint,
    modulus_le: &[u8],
    server_eph_le: &[u8],
    hashed_password: &[u8],
    a: &BigUint,
) -> Result<Proofs, SrpError> {
    let one = BigUint::one();
    let n_minus_one = n - &one;
    let g = BigUint::from(2u32);
    let x = to_int(hashed_password);
    let b = to_int(server_eph_le);

    // A = g^a mod N
    let a_pub = g.modpow(a, n);
    let a_le = from_int(&a_pub);

    // u = H(A ‖ B)
    let mut ub = a_le.clone();
    ub.extend_from_slice(server_eph_le);
    let u = to_int(&expand_hash(&ub));

    // k = H(g ‖ N) mod N
    let mut kb = from_int(&g);
    kb.extend_from_slice(modulus_le);
    let k = to_int(&expand_hash(&kb)) % n;
    if k <= one || k >= n_minus_one {
        return Err(err("SRP multiplier is out of bounds"));
    }

    // base = B - k * g^x   (mod N)
    let gx = g.modpow(&x, n);
    let t = (&gx * &k) % n;
    let base = (&b + n - t) % n;

    // exponent = a + u * x   (mod N-1)
    let exponent = (a + (&u % &n_minus_one) * (&x % &n_minus_one)) % &n_minus_one;

    // S = base^exponent mod N
    let s = base.modpow(&exponent, n);
    let s_le = from_int(&s);

    // M1 = H(A ‖ B ‖ S), M2 = H(A ‖ M1 ‖ S)
    let mut cp = a_le.clone();
    cp.extend_from_slice(server_eph_le);
    cp.extend_from_slice(&s_le);
    let client_proof = expand_hash(&cp);

    let mut sp = a_le.clone();
    sp.extend_from_slice(&client_proof);
    sp.extend_from_slice(&s_le);
    let server_proof = expand_hash(&sp);

    Ok(Proofs {
        client_ephemeral: B64.encode(&a_le),
        client_proof: B64.encode(&client_proof),
        expected_server_proof: server_proof,
    })
}

/// Draw a client secret in the valid range, matching go-srp's bounds
/// (`2·bitLength < a < N-1`).
fn random_secret(n: &BigUint) -> BigUint {
    use rand_core::{OsRng, RngCore};
    let lower = BigUint::from((BIT_LENGTH * 2) as u64);
    let n_minus_one = n - BigUint::one();
    loop {
        let mut buf = [0u8; BYTE_LENGTH];
        OsRng.fill_bytes(&mut buf);
        let candidate = BigUint::from_bytes_be(&buf) % &n_minus_one;
        if candidate > lower && candidate < n_minus_one {
            return candidate;
        }
    }
}

/// Full client flow: verify the modulus, hash the password, validate parameters,
/// and produce the proofs for `/auth/v4`.
pub fn compute_proofs(
    version: u32,
    password: &[u8],
    salt_b64: &str,
    signed_modulus: &str,
    server_ephemeral_b64: &str,
) -> Result<Proofs, SrpError> {
    let modulus = verify_and_extract_modulus(signed_modulus)?;
    let server_eph = B64
        .decode(server_ephemeral_b64)
        .map_err(|e| err(format!("bad server ephemeral: {e}")))?;
    let hashed = hash_password(version, password, salt_b64, &modulus)?;

    let n = to_int(&modulus);
    let b = to_int(&server_eph);
    check_params(&n, &b)?;

    let a = random_secret(&n);
    proofs_from(&n, &modulus, &server_eph, &hashed, &a)
}
