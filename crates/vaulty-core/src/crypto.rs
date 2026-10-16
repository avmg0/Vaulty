//! Small, audited building blocks only: Argon2id, XChaCha20-Poly1305, X25519 sealed boxes,
//! SHA-256, OS randomness. Keys live in `Key`, which wipes itself from memory when dropped.
use crate::error::{Error, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use zeroize::Zeroizing;
use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

/// A 256-bit secret key, wiped from memory on drop.
pub type Key = Zeroizing<[u8; 32]>;

pub fn new_key() -> Key { Zeroizing::new(random_bytes::<32>()) }

/// Argon2id with 64 MiB of memory and 3 passes: above OWASP's minimum, still well under a
/// second on a phone. Every password, setup code and recovery key goes through this.
fn argon2() -> Argon2<'static> {
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(64 * 1024, 3, 1, Some(32)).expect("valid Argon2 params"))
}

pub fn hash_secret(secret: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| Error::Auth(e.to_string()))
}

/// True when a stored hash already uses the current Argon2 parameters.
pub fn is_current_hash(phc: &str) -> bool {
    phc.starts_with("$argon2id$") && phc.contains("m=65536,t=3,p=1")
}

pub fn verify_secret(secret: &str, phc: &str) -> bool {
    // Parameters come from the stored hash, so older hashes still verify.
    PasswordHash::new(phc)
        .map(|h| Argon2::default().verify_password(secret.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// Key-encryption key from a password, setup code or recovery key (Argon2id, strong params).
pub fn derive_key(secret: &str, salt_hex: &str) -> Result<Key> {
    let salt = from_hex(salt_hex)?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2().hash_password_into(secret.as_bytes(), &salt, key.as_mut()).map_err(|e| Error::Auth(e.to_string()))?;
    Ok(key)
}

/// The Argon2 defaults used before data encryption (only to unseal old 2FA secrets once).
pub fn derive_key_legacy(password: &str, salt_hex: &str) -> Result<Key> {
    let salt = from_hex(salt_hex)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::default().hash_password_into(password.as_bytes(), &salt, key.as_mut()).map_err(|e| Error::Auth(e.to_string()))?;
    Ok(key)
}

pub fn new_salt() -> String { to_hex(&random_bytes::<16>()) }

/// Seals a key under another key (XChaCha20-Poly1305).
pub fn wrap(kek: &Key, key: &Key) -> Result<String> { seal(kek, key.as_ref()) }

/// Opens a wrapped key. Fails if `kek` is wrong or the value was tampered with.
pub fn unwrap(kek: &Key, wrapped: &str) -> Result<Key> {
    let plain = Zeroizing::new(open(kek, wrapped)?);
    let arr: [u8; 32] = plain.as_slice().try_into().map_err(|_| Error::Auth("wrapped key is corrupt".into()))?;
    Ok(Zeroizing::new(arr))
}

/// X25519 key pair for receiving keys from other members. Returns (public hex, secret).
pub fn new_keypair() -> (String, Key) {
    let secret = crypto_box::SecretKey::generate(&mut OsRng);
    (to_hex(secret.public_key().as_bytes()), Zeroizing::new(secret.to_bytes()))
}

/// Seals `key` so only the holder of `public_hex`'s secret key can open it (anonymous sealed box).
pub fn seal_to(public_hex: &str, key: &Key) -> Result<String> {
    let pk: [u8; 32] = from_hex(public_hex)?.try_into().map_err(|_| Error::Invalid("bad public key".into()))?;
    let ct = crypto_box::PublicKey::from(pk).seal(&mut OsRng, key.as_ref()).map_err(|_| Error::Auth("encryption failed".into()))?;
    Ok(to_hex(&ct))
}

pub fn open_sealed(secret: &Key, sealed_hex: &str) -> Result<Key> {
    let sk = crypto_box::SecretKey::from(**secret);
    let plain = Zeroizing::new(sk.unseal(&from_hex(sealed_hex)?).map_err(|_| Error::Auth("could not open shared key".into()))?);
    let arr: [u8; 32] = plain.as_slice().try_into().map_err(|_| Error::Auth("shared key is corrupt".into()))?;
    Ok(Zeroizing::new(arr))
}

pub fn seal(key: &Key, plaintext: &[u8]) -> Result<String> {
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ct = XChaCha20Poly1305::new(key.as_ref().into())
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| Error::Auth("encryption failed".into()))?;
    Ok(to_hex(&[nonce.as_slice(), &ct].concat()))
}

pub fn open(key: &Key, sealed_hex: &str) -> Result<Vec<u8>> {
    let raw = from_hex(sealed_hex)?;
    if raw.len() < 24 {
        return Err(Error::Auth("sealed value is corrupt".into()));
    }
    let (nonce, ct) = raw.split_at(24);
    XChaCha20Poly1305::new(key.as_ref().into())
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
