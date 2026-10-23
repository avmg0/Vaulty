//! Shamir secret sharing over GF(2^8), byte by byte: a secret split into `n` shares so that any
//! `k` of them give it back and fewer than `k` say nothing about it.
//!
//! Used for shared recovery (any 2 of 3 helpers). Each share is `(x, bytes)` with `x` in 1..=255.
//! Arithmetic is in the AES field (x^8 + x^4 + x^3 + x + 1) without lookup tables, so timing
//! doesn't depend on the secret's bytes.
use crate::error::{Error, Result};
use zeroize::Zeroizing;

/// Multiplication in GF(2^8), constant time.
fn mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        p ^= a & (0u8.wrapping_sub(b & 1));
        let carry = 0u8.wrapping_sub(a >> 7);
        a = (a << 1) ^ (0x1b & carry);
        b >>= 1;
    }
    p
}

/// a^254 = a^-1 (and 0 for 0).
fn inv(a: u8) -> u8 {
    let mut r = 1u8;
    let mut base = a;
    let mut e = 254u8;
    while e > 0 {
        if e & 1 == 1 { r = mul(r, base); }
        base = mul(base, base);
        e >>= 1;
    }
    r
}

#[derive(Clone)]
pub struct Share {
    pub x: u8,
    pub bytes: Zeroizing<Vec<u8>>,
}

/// Splits `secret` into `n` shares, any `k` of which rebuild it.
pub fn split(secret: &[u8], k: u8, n: u8) -> Result<Vec<Share>> {
    if k < 2 || n < k { return Err(Error::Invalid("need at least 2 shares, and no fewer shares than needed".into())); }
    let mut shares: Vec<Share> = (1..=n).map(|x| Share { x, bytes: Zeroizing::new(Vec::with_capacity(secret.len())) }).collect();
    for &s in secret {
        // A random polynomial of degree k-1 with the secret byte as its constant term.
        let mut coeffs = Zeroizing::new(vec![s]);
        coeffs.extend((1..k).map(|_| crate::crypto::random_bytes::<1>()[0]));
        for share in &mut shares {
            // Horner's rule.
            let y = coeffs.iter().rev().fold(0u8, |acc, &c| mul(acc, share.x) ^ c);
            share.bytes.push(y);
        }
    }
    Ok(shares)
}

/// The share at `x` of the line through `secret` with the given slope (2-of-n sharing). The same
/// secret and slope always give the same share, so a share can be issued again later.
pub fn share_at(secret: &[u8], slope: &[u8], x: u8) -> Result<Share> {
    if x == 0 || secret.len() != slope.len() { return Err(Error::Invalid("those shares don't fit together".into())); }
    Ok(Share { x, bytes: Zeroizing::new(secret.iter().zip(slope).map(|(&c0, &c1)| c0 ^ mul(c1, x)).collect()) })
}

/// Rebuilds the secret from at least `k` distinct shares (extra ones are fine if they agree;
/// the caller checks the result, e.g. by unwrapping a key with it).
pub fn combine(shares: &[Share]) -> Result<Zeroizing<Vec<u8>>> {
    let bad = || Error::Invalid("those shares don't fit together".into());
    let Some(first) = shares.first() else { return Err(bad()) };
    let len = first.bytes.len();
    if shares.len() < 2 || shares.iter().any(|s| s.x == 0 || s.bytes.len() != len) { return Err(bad()); }
    for (i, a) in shares.iter().enumerate() {
        if shares[i + 1..].iter().any(|b| b.x == a.x) { return Err(bad()); }
    }
    // Lagrange interpolation at x = 0.
    let weights: Vec<u8> = shares.iter().map(|a| {
        shares.iter().filter(|b| b.x != a.x).fold(1u8, |w, b| mul(w, mul(b.x, inv(b.x ^ a.x))))
    }).collect();
    let mut out = Zeroizing::new(vec![0u8; len]);
    for (s, w) in shares.iter().zip(&weights) {
        for (o, &y) in out.iter_mut().zip(s.bytes.iter()) { *o ^= mul(y, *w); }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_inverse() {
        for a in 1..=255u8 { assert_eq!(mul(a, inv(a)), 1, "{a}"); }
        assert_eq!(mul(0x57, 0x83), 0xc1, "FIPS-197 example");
    }

    #[test]
    fn any_two_of_three() {
        let secret = b"sixteen byte key";
        let s = split(secret, 2, 3).unwrap();
        for (a, b) in [(0, 1), (0, 2), (1, 2), (2, 0)] {
            assert_eq!(&combine(&[s[a].clone(), s[b].clone()]).unwrap()[..], secret);
        }
        assert_eq!(&combine(&s).unwrap()[..], secret, "all three agree");
        assert!(combine(&s[..1]).is_err(), "one isn't enough");
        assert!(combine(&[s[0].clone(), s[0].clone()]).is_err(), "the same share twice");
        // A wrong share gives a wrong secret (caught by the key it unlocks).
        let mut wrong = s[1].clone();
        wrong.bytes[0] ^= 1;
        assert_ne!(&combine(&[s[0].clone(), wrong]).unwrap()[..], secret);
    }

    #[test]
    fn shares_on_a_known_line() {
        let (secret, slope) = (b"0123456789abcdef", b"fedcba9876543210");
        let shares: Vec<Share> = (1..=4).map(|x| share_at(secret, slope, x).unwrap()).collect();
        assert_eq!(&combine(&[shares[3].clone(), shares[1].clone()]).unwrap()[..], secret, "a fourth share fits the same line");
        assert_eq!(share_at(secret, slope, 4).unwrap().bytes, shares[3].bytes, "issued again, the same");
    }

    #[test]
    fn one_share_reveals_nothing() {
        // For a fixed share x=1, every secret byte is equally likely to produce any share byte.
        let mut seen = [false; 256];
        for _ in 0..4000 { seen[split(&[0x42], 2, 3).unwrap()[0].bytes[0] as usize] = true; }
        assert!(seen.iter().filter(|s| **s).count() > 240);
    }
}
