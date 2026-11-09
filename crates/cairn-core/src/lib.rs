//! # CAIRN
//!
//! **C**onstant-time, **A**uthenticated, **I**nformation-theoretic,
//! **R**efreshable, i**N**ner-product custody for master keys.
//!
//! CAIRN keeps a master key under `t`-of-`n` custody using nothing but linear
//! algebra over the Mersenne prime field GF(2^127 - 1). There is no block
//! cipher, no hash function and no hardness assumption anywhere in the
//! scheme: every security property is an unconditional theorem with an
//! explicit probability bound (see `paper/cairn.pdf`).
//!
//! The pipeline, from secret to stored shares:
//!
//! | layer | module | what it does | window it closes |
//! |---|---|---|---|
//! | L1 | [`encode`] | bytes to 120-bit limbs | ambiguity / malleable parsing |
//! | L2 | [`amd`] | algebraic manipulation detection tag | tampering, Tompa-Woll cheating |
//! | L3 | [`poly`] | Shamir-type polynomial sharing | theft of up to `t-1` custodians |
//! | L4 | [`ipenc`] | inner-product split-state storage | bounded memory / side-channel leakage |
//! | L5 | [`scheme`] refresh | zero-sharing + orthogonal re-randomisation | mobile adversary over time |
//! | L6 | [`scheme`] combine | constant-time masked reconstruction | timing, cache, share materialisation |

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod amd;
pub mod ct;
pub mod encode;
pub mod field;
pub mod format;
pub mod ipenc;
pub mod poly;
pub mod scheme;

#[cfg(any(test, feature = "insecure-test-rng"))]
pub mod testrng;

pub use field::{Field, Fp127, Toy};
pub use scheme::{Delta, Meta, Params, Recovery, Share};

use alloc::vec::Vec;
use zeroize::Zeroizing;

/// Everything that can go wrong. Variants carry no secret data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `t`, `n` or `ell` out of range.
    InvalidParams(&'static str),
    /// Secret longer than the 65535-byte encoding limit.
    SecretTooLong,
    /// Fewer than `t` shares supplied.
    NotEnoughShares {
        /// Threshold required by the shares.
        needed: u8,
        /// Number of usable shares supplied.
        got: usize,
    },
    /// Two shares carry the same index.
    DuplicateIndex(u8),
    /// Shares come from different dealings or have different parameters.
    Mismatch(&'static str),
    /// Shares come from different refresh epochs.
    EpochMismatch {
        /// Lowest epoch seen.
        min: u32,
        /// Highest epoch seen.
        max: u32,
    },
    /// The algebraic manipulation detection check failed for every subset.
    IntegrityFailure,
    /// A file or buffer is not a valid CAIRN object.
    Malformed(&'static str),
    /// The randomness source failed. CAIRN never falls back to weak randomness.
    Rng,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::InvalidParams(m) => write!(f, "invalid parameters: {m}"),
            Error::SecretTooLong => f.write_str("secret longer than 65535 bytes"),
            Error::NotEnoughShares { needed, got } => {
                write!(f, "need {needed} shares, got {got}")
            }
            Error::DuplicateIndex(i) => write!(f, "share index {i} supplied twice"),
            Error::Mismatch(m) => write!(f, "shares do not belong together: {m}"),
            Error::EpochMismatch { min, max } => {
                write!(f, "shares are from different refresh epochs ({min} to {max})")
            }
            Error::IntegrityFailure => {
                f.write_str("integrity check failed: shares were tampered with or are inconsistent")
            }
            Error::Malformed(m) => write!(f, "malformed input: {m}"),
            Error::Rng => f.write_str("randomness source failed"),
        }
    }
}

/// A source of uniformly random bytes.
///
/// Production code passes the operating system CSPRNG (`getrandom(2)`).
/// The trait exists so that the exact same algorithms can be driven by a
/// deterministic stream for known-answer tests and the lab.
pub trait RandomSource {
    /// Fill `dest` with uniformly random bytes, or fail closed.
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error>;
}

impl<R: RandomSource + ?Sized> RandomSource for &mut R {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        (**self).fill_bytes(dest)
    }
}

/// Split a byte secret (for example a 32-byte master key) into `n` shares.
///
/// This is the main entry point: encode (L1), armor (L2), share (L3) and
/// store each share as inner-product pairs (L4).
pub fn split<R: RandomSource + ?Sized>(
    secret: &[u8],
    params: Params,
    rng: &mut R,
) -> Result<Vec<Share<Fp127>>, Error> {
    let limbs = encode::encode(secret, params.min_limbs)?;
    scheme::split_vector(&limbs, params, rng)
}

/// Recover the byte secret from `t` or more shares.
///
/// Returns the secret together with the indices that were used and the
/// indices that were found inconsistent (tampered or stale).
pub fn combine(shares: &[Share<Fp127>]) -> Result<(Zeroizing<Vec<u8>>, Recovery<Fp127>), Error> {
    let rec = scheme::combine_vector(shares)?;
    let bytes = encode::decode(&rec.value)?;
    Ok((bytes, rec))
}
