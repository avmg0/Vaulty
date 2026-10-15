//! Small, audited building blocks only: Argon2id, XChaCha20-Poly1305, SHA-256, OS randomness.
use crate::error::{Error, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

pub fn hash_secret(secret: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| Error::Auth(e.to_string()))
}

pub fn verify_secret(secret: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| Argon2::default().verify_password(secret.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// 32-byte key from a password (Argon2id), used to seal per-user secrets at rest.
pub fn derive_key(password: &str, salt_hex: &str) -> Result<[u8; 32]> {
    let salt = from_hex(salt_hex)?;
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| Error::Auth(e.to_string()))?;
    Ok(key)
}

pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<String> {
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ct = XChaCha20Poly1305::new(key.into())
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| Error::Auth("encryption failed".into()))?;
    Ok(to_hex(&[nonce.as_slice(), &ct].concat()))
}

pub fn open(key: &[u8; 32], sealed_hex: &str) -> Result<Vec<u8>> {
    let raw = from_hex(sealed_hex)?;
    if raw.len() < 24 {
        return Err(Error::Auth("sealed value is corrupt".into()));
    }
    let (nonce, ct) = raw.split_at(24);
    XChaCha20Poly1305::new(key.into())
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| Error::Auth("could not unseal secret".into()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    to_hex(&Sha256::digest(data))
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}

/// Random code from an alphabet without look-alike characters (no 0/O, 1/I/L).
pub fn random_code(len: usize, alphabet: &[u8]) -> String {
    // Rejection sampling keeps the distribution uniform.
    let limit = 256 - (256 % alphabet.len());
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let b = random_bytes::<1>()[0] as usize;
        if b < limit {
            out.push(alphabet[b % alphabet.len()] as char);
        }
    }
    out
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err(Error::Invalid("bad hex".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| Error::Invalid("bad hex".into())))
        .collect()
}
