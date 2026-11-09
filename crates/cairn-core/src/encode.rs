//! L1: injective encoding of a byte string into field limbs.
//!
//! Layout (normative):
//!
//! ```text
//! buf  = len_u16_be || secret || 0x00 ... 0x00      (padded to 15*k bytes)
//! s_i  = big-endian integer of buf[15(i-1) .. 15i]  (so 0 <= s_i < 2^120 < p)
//! ```
//!
//! `k = max(ceil((len + 2) / 15), min_limbs)`. Setting `min_limbs` pads short
//! secrets to a common size so that the number of limbs, which is public,
//! does not reveal the secret's length. Text secrets are plain UTF-8 bytes.

use alloc::vec;
use alloc::vec::Vec;
use zeroize::{Zeroize, Zeroizing};

use core::ops::Not;

use crate::ct::ct_is_nonzero_u128;
use crate::field::Fp127;
use crate::{Error, Field};

/// Bytes per limb. 120 bits is the largest whole number of bytes below 127 bits.
pub const LIMB_BYTES: usize = 15;
/// Maximum secret length in bytes.
pub const MAX_SECRET_LEN: usize = u16::MAX as usize;

/// Number of limbs needed for a secret of `len` bytes.
pub fn limbs_for(len: usize, min_limbs: usize) -> usize {
    (len + 2).div_ceil(LIMB_BYTES).max(min_limbs).max(1)
}

/// Encode `secret` into `k` limbs.
pub fn encode(secret: &[u8], min_limbs: usize) -> Result<Zeroizing<Vec<Fp127>>, Error> {
    if secret.len() > MAX_SECRET_LEN {
        return Err(Error::SecretTooLong);
    }
    let k = limbs_for(secret.len(), min_limbs);
    let mut buf = Zeroizing::new(vec![0u8; k * LIMB_BYTES]);
    buf[..2].copy_from_slice(&(secret.len() as u16).to_be_bytes());
    buf[2..2 + secret.len()].copy_from_slice(secret);

    let mut out = Zeroizing::new(Vec::with_capacity(k));
    for chunk in buf.chunks_exact(LIMB_BYTES) {
        let mut wide = [0u8; 16];
        wide[1..].copy_from_slice(chunk);
        out.push(Fp127::from_raw(u128::from_be_bytes(wide)));
        wide.zeroize();
    }
    Ok(out)
}

/// Decode limbs back to the secret, rejecting anything that is not the exact
/// output of [`encode`] (limb out of range, bad length, non-zero padding).
///
/// Secret bytes never drive a branch: the range and padding checks are
/// accumulated with OR and only the final verdict is declassified. The
/// length prefix *is* declassified, because the length of the returned
/// secret is visible to the caller anyway.
pub fn decode(limbs: &[Fp127]) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut buf = Zeroizing::new(Vec::with_capacity(limbs.len() * LIMB_BYTES));
    let mut bad: u8 = 0;
    for limb in limbs {
        let mut wide = limb.to_u128().to_be_bytes();
        bad |= wide[0];
        buf.extend_from_slice(&wide[1..]);
        wide.zeroize();
    }
    if buf.len() < 2 {
        return Err(Error::Malformed("empty encoding"));
    }
    let len = u16::from_be_bytes([buf[0], buf[1]]) as usize; // declassified
    if len + 2 > buf.len() {
        return Err(Error::Malformed("length prefix exceeds encoding"));
    }
    for &b in &buf[2 + len..] {
        bad |= b;
    }
    if !ct_is_nonzero_u128(bad as u128).not().declassify() {
        return Err(Error::Malformed("limb out of range or non-zero padding"));
    }
    Ok(Zeroizing::new(buf[2..2 + len].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip(secret in proptest::collection::vec(any::<u8>(), 0..300), min in 0usize..8) {
            let limbs = encode(&secret, min).unwrap();
            prop_assert_eq!(limbs.len(), limbs_for(secret.len(), min));
            prop_assert_eq!(&decode(&limbs).unwrap()[..], &secret[..]);
        }
    }

    #[test]
    fn master_key_uses_three_limbs() {
        assert_eq!(limbs_for(32, 0), 3);
    }

    #[test]
    fn utf8_text_roundtrips() {
        let s = "clé maîtresse 🔑 مفتاح";
        let limbs = encode(s.as_bytes(), 0).unwrap();
        assert_eq!(core::str::from_utf8(&decode(&limbs).unwrap()).unwrap(), s);
    }

    #[test]
    fn rejects_non_encodings() {
        let mut limbs = encode(b"key", 0).unwrap();
        limbs[0] = Fp127::from_raw(1u128 << 121);
        assert!(decode(&limbs).is_err());
        let limbs = [Fp127::from_raw(0xFFFF << 104)]; // length 65535 in a 1-limb buffer
        assert!(decode(&limbs).is_err());
        let limbs = [Fp127::from_raw(1)]; // length 0 but padding byte 1
        assert!(decode(&limbs).is_err());
    }

    #[test]
    fn too_long() {
        assert_eq!(encode(&vec![0u8; MAX_SECRET_LEN + 1], 0).unwrap_err(), Error::SecretTooLong);
    }
}
