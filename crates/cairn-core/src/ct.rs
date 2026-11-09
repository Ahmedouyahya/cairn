//! Constant-time building blocks.
//!
//! Every function here is branch-free with respect to its arguments. Values
//! are routed through [`core::hint::black_box`] at the boundary between
//! "mask arithmetic" and "use of the mask", which stops LLVM from recognising
//! the pattern and turning it back into a conditional jump. This is the same
//! defensive idea used by the `subtle` crate, written out here so that the
//! whole trusted computing base of CAIRN is visible in one repository.

use core::hint::black_box;
use core::ops::Not;

/// A secret boolean, stored as `0` or `1`.
///
/// `Choice` deliberately does not implement `PartialEq` or conversion to
/// `bool`: turning a secret predicate into control flow must be an explicit,
/// visible act ([`Choice::declassify`]).
#[derive(Clone, Copy, Debug)]
pub struct Choice(u8);

impl Choice {
    /// The secret value "true".
    pub const TRUE: Choice = Choice(1);
    /// The secret value "false".
    pub const FALSE: Choice = Choice(0);

    /// Build a choice from the lowest bit of `bit`.
    #[inline]
    pub fn from_bit(bit: u8) -> Choice {
        Choice(black_box(bit & 1))
    }

    /// Logical and.
    #[inline]
    #[must_use]
    pub fn and(self, other: Choice) -> Choice {
        Choice(self.0 & other.0)
    }

    /// Logical or.
    #[inline]
    #[must_use]
    pub fn or(self, other: Choice) -> Choice {
        Choice(self.0 | other.0)
    }

    /// All-ones if true, all-zeros if false.
    #[inline]
    pub fn mask_u128(self) -> u128 {
        0u128.wrapping_sub(black_box(self.0) as u128)
    }

    /// All-ones if true, all-zeros if false.
    #[inline]
    pub fn mask_u64(self) -> u64 {
        0u64.wrapping_sub(black_box(self.0) as u64)
    }

    /// Turn the secret predicate into a public `bool`.
    ///
    /// Call this only on values whose disclosure is part of the protocol,
    /// for example "did the integrity check pass".
    #[inline]
    pub fn declassify(self) -> bool {
        black_box(self.0) == 1
    }
}

impl Not for Choice {
    type Output = Choice;
    /// Logical not.
    #[inline]
    fn not(self) -> Choice {
        Choice(self.0 ^ 1)
    }
}

/// `1` if `x != 0`, else `0`, computed without branches.
///
/// For any `x != 0`, at least one of `x` and `-x` has its top bit set.
#[inline]
pub fn ct_is_nonzero_u128(x: u128) -> Choice {
    Choice::from_bit(((x | x.wrapping_neg()) >> 127) as u8)
}

/// Constant-time equality of two `u128`.
#[inline]
pub fn ct_eq_u128(a: u128, b: u128) -> Choice {
    ct_is_nonzero_u128(a ^ b).not()
}

/// Returns `a` if `c` is true and `b` otherwise.
#[inline]
pub fn ct_select_u128(c: Choice, a: u128, b: u128) -> u128 {
    let m = c.mask_u128();
    b ^ (m & (a ^ b))
}

/// Constant-time equality of two byte strings of equal, public length.
///
/// Returns false (without inspecting contents) if the lengths differ,
/// because lengths are public in every use inside CAIRN.
pub fn ct_eq_bytes(a: &[u8], b: &[u8]) -> Choice {
    if a.len() != b.len() {
        return Choice::FALSE;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    ct_is_nonzero_u128(black_box(acc) as u128).not()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_edge_cases() {
        for x in [0u128, 1, 2, u128::MAX, 1 << 127, (1 << 127) - 1, 1 << 64] {
            assert_eq!(ct_is_nonzero_u128(x).declassify(), x != 0, "x = {x}");
        }
    }

    #[test]
    fn select_and_eq() {
        assert_eq!(ct_select_u128(Choice::TRUE, 7, 9), 7);
        assert_eq!(ct_select_u128(Choice::FALSE, 7, 9), 9);
        assert!(ct_eq_u128(5, 5).declassify());
        assert!(!ct_eq_u128(5, 6).declassify());
        assert!(ct_eq_bytes(b"abc", b"abc").declassify());
        assert!(!ct_eq_bytes(b"abc", b"abd").declassify());
        assert!(!ct_eq_bytes(b"abc", b"ab").declassify());
    }

    #[test]
    fn choice_logic() {
        let (t, f) = (Choice::TRUE, Choice::FALSE);
        assert!(t.and(t).declassify() && !t.and(f).declassify());
        assert!(t.or(f).declassify() && !f.or(f).declassify());
        assert!(f.not().declassify() && !t.not().declassify());
        assert_eq!(t.mask_u128(), u128::MAX);
        assert_eq!(f.mask_u64(), 0);
    }
}
