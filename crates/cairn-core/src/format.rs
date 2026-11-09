//! Canonical binary encoding of shares and refresh deltas.
//!
//! ```text
//! offset size  field
//! 0      5     magic  "CAIRN"
//! 5      1     version = 1
//! 6      1     kind: 1 = share, 2 = delta
//! 7      16    set_id
//! 23     1     t
//! 24     1     n
//! 25     1     index
//! 26     1     ell
//! 27     2     dim     (little-endian)
//! 29     4     epoch   (little-endian)
//! 33     ...   body
//! share body: L half (dim*ell elements) then R half (dim*ell elements)
//! delta body: dim elements
//! element: 16 bytes little-endian, must be < 2^127 - 1
//! ```
//!
//! The parser is strict: exact length, known version and kind, canonical
//! field elements, index in `1..=n`, `t <= n`. Every byte string has at most
//! one meaning, so there is no parsing ambiguity for an attacker to exploit.

use alloc::vec::Vec;
use zeroize::{Zeroize, Zeroizing};

use crate::Error;
use crate::field::Fp127;
use crate::scheme::{Delta, Meta, Share};

/// File magic.
pub const MAGIC: &[u8; 5] = b"CAIRN";
/// Format version.
pub const VERSION: u8 = 1;
/// Header length in bytes.
pub const HEADER_LEN: usize = 33;
const KIND_SHARE: u8 = 1;
const KIND_DELTA: u8 = 2;

fn write_header(out: &mut Vec<u8>, kind: u8, m: &Meta) {
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(kind);
    out.extend_from_slice(&m.set_id);
    out.extend_from_slice(&[m.t, m.n, m.index, m.ell]);
    out.extend_from_slice(&m.dim.to_le_bytes());
    out.extend_from_slice(&m.epoch.to_le_bytes());
}

fn read_header(b: &[u8], kind: u8) -> Result<Meta, Error> {
    if b.len() < HEADER_LEN {
        return Err(Error::Malformed("truncated header"));
    }
    if &b[..5] != MAGIC {
        return Err(Error::Malformed("not a CAIRN file"));
    }
    if b[5] != VERSION {
        return Err(Error::Malformed("unsupported version"));
    }
    if b[6] != kind {
        return Err(Error::Malformed("wrong object kind"));
    }
    let mut set_id = [0u8; 16];
    set_id.copy_from_slice(&b[7..23]);
    let m = Meta {
        set_id,
        t: b[23],
        n: b[24],
        index: b[25],
        ell: b[26],
        dim: u16::from_le_bytes([b[27], b[28]]),
        epoch: u32::from_le_bytes([b[29], b[30], b[31], b[32]]),
    };
    if m.t == 0 || m.t > m.n || m.index == 0 || m.index > m.n {
        return Err(Error::Malformed("bad t, n or index"));
    }
    if m.ell == 0 || m.ell > 64 || m.dim < 3 {
        return Err(Error::Malformed("bad ell or dim"));
    }
    Ok(m)
}

fn write_elems(out: &mut Vec<u8>, xs: &[Fp127]) {
    for x in xs {
        let mut b = x.to_le_bytes();
        out.extend_from_slice(&b);
        b.zeroize();
    }
}

fn read_elems(b: &[u8]) -> Result<Vec<Fp127>, Error> {
    let mut out = Vec::with_capacity(b.len() / 16);
    for chunk in b.chunks_exact(16) {
        let mut a = [0u8; 16];
        a.copy_from_slice(chunk);
        let x = Fp127::from_le_bytes(a);
        a.zeroize();
        match x {
            Some(x) => out.push(x),
            None => {
                out.zeroize();
                return Err(Error::Malformed("non-canonical field element"));
            }
        }
    }
    Ok(out)
}

/// Serialize a share.
pub fn share_to_bytes(s: &Share<Fp127>) -> Zeroizing<Vec<u8>> {
    let mut out = Vec::with_capacity(HEADER_LEN + 32 * s.left().len());
    write_header(&mut out, KIND_SHARE, &s.meta);
    write_elems(&mut out, s.left());
    write_elems(&mut out, s.right());
    Zeroizing::new(out)
}

/// Parse a share.
pub fn share_from_bytes(b: &[u8]) -> Result<Share<Fp127>, Error> {
    let m = read_header(b, KIND_SHARE)?;
    let half = m.dim as usize * m.ell as usize * 16;
    if b.len() != HEADER_LEN + 2 * half {
        return Err(Error::Malformed("share has the wrong length"));
    }
    let left = read_elems(&b[HEADER_LEN..HEADER_LEN + half])?;
    let right = read_elems(&b[HEADER_LEN + half..])?;
    Share::from_parts(m, left, right)
}

/// Serialize a refresh delta.
pub fn delta_to_bytes(d: &Delta<Fp127>) -> Zeroizing<Vec<u8>> {
    let mut out = Vec::with_capacity(HEADER_LEN + 16 * d.values.len());
    write_header(&mut out, KIND_DELTA, &d.meta);
    write_elems(&mut out, &d.values);
    Zeroizing::new(out)
}

/// Parse a refresh delta.
pub fn delta_from_bytes(b: &[u8]) -> Result<Delta<Fp127>, Error> {
    let m = read_header(b, KIND_DELTA)?;
    if b.len() != HEADER_LEN + m.dim as usize * 16 {
        return Err(Error::Malformed("delta has the wrong length"));
    }
    let values = Zeroizing::new(read_elems(&b[HEADER_LEN..])?);
    Ok(Delta { meta: m, values })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::{Params, refresh_deltas};
    use crate::testrng::SplitMix64;

    #[test]
    fn share_roundtrip_and_strictness() {
        let mut rng = SplitMix64::new(21);
        let shares =
            crate::split(b"0123456789abcdef0123456789abcdef", Params::new(2, 3), &mut rng).unwrap();
        let bytes = share_to_bytes(&shares[1]);
        let back = share_from_bytes(&bytes).unwrap();
        assert_eq!(back.meta, shares[1].meta);
        assert_eq!(&share_to_bytes(&back)[..], &bytes[..]);

        // Every truncation and every extension is rejected.
        for cut in [0, 5, HEADER_LEN, bytes.len() - 1] {
            assert!(share_from_bytes(&bytes[..cut]).is_err());
        }
        let mut longer = bytes.to_vec();
        longer.push(0);
        assert!(share_from_bytes(&longer).is_err());

        // A non-canonical element (all ones) is rejected.
        let mut bad = bytes.to_vec();
        bad[HEADER_LEN..HEADER_LEN + 16].fill(0xFF);
        assert!(share_from_bytes(&bad).is_err());

        // Index 0 is rejected.
        let mut bad = bytes.to_vec();
        bad[25] = 0;
        assert!(share_from_bytes(&bad).is_err());
    }

    #[test]
    fn delta_roundtrip() {
        let mut rng = SplitMix64::new(22);
        let shares = crate::split(b"k", Params::new(2, 3), &mut rng).unwrap();
        let deltas = refresh_deltas::<Fp127, _>(&shares[0].meta, &mut rng).unwrap();
        let b = delta_to_bytes(&deltas[2]);
        let d = delta_from_bytes(&b).unwrap();
        assert_eq!(d.meta, deltas[2].meta);
        assert!(share_from_bytes(&b).is_err(), "kinds must not be confused");
    }
}
