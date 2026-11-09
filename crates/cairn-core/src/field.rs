//! Prime fields.
//!
//! * [`Fp127`] is the production field GF(p) with the Mersenne prime
//!   p = 2^127 - 1. Because 2^127 = 1 (mod p), reduction is a shift and an
//!   add, so every operation is a fixed sequence of 64x64 multiplies, adds
//!   and masks: no division, no data-dependent branch, no table lookup.
//! * [`Toy`] is a tiny prime field used only by the lab to verify the
//!   security theorems *exhaustively* at small parameters. It uses `%`, is
//!   not constant time and must never hold a real secret.
//!
//! All CAIRN algorithms are generic over [`Field`], so the exact code that
//! protects real keys is the code that the exhaustive checks run.

use core::fmt;
use core::ops::Not;
use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use zeroize::Zeroize;

use crate::ct::{Choice, ct_eq_u128, ct_is_nonzero_u128, ct_select_u128};
use crate::{Error, RandomSource};

/// A prime field with constant-time operations.
pub trait Field:
    Copy
    + Default
    + Zeroize
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + fmt::Debug
    + 'static
{
    /// The prime modulus.
    const MODULUS: u128;
    /// Canonical serialized length in bytes.
    const BYTES: usize;

    /// Additive identity.
    fn zero() -> Self;
    /// Multiplicative identity.
    fn one() -> Self;
    /// Embed a public small integer.
    fn from_u64(x: u64) -> Self;
    /// Constant-time equality.
    fn ct_eq(&self, other: &Self) -> Choice;
    /// Constant-time `self == 0`.
    fn ct_is_zero(&self) -> Choice;
    /// Returns `a` if `c` is true, `b` otherwise, without branching.
    fn ct_select(c: Choice, a: &Self, b: &Self) -> Self;
    /// Draw a uniformly random element.
    fn random<R: RandomSource + ?Sized>(rng: &mut R) -> Result<Self, Error>;
    /// Draw a uniformly random non-zero element.
    fn random_nonzero<R: RandomSource + ?Sized>(rng: &mut R) -> Result<Self, Error> {
        loop {
            let x = Self::random(rng)?;
            // Rejection happens with probability 1/p. Revealing that a
            // rejected candidate was zero reveals nothing about the accepted one.
            if !x.ct_is_zero().declassify() {
                return Ok(x);
            }
        }
    }
    /// The canonical integer representative in `[0, p)`.
    ///
    /// This is a plain conversion, not a comparison: it does not leak.
    fn to_u128(&self) -> u128;

    /// `self^e` for a **public** exponent `e`.
    ///
    /// Left-to-right square and multiply: the sequence of operations depends
    /// only on the bits of `e`, never on `self`.
    fn pow_public(&self, e: u128) -> Self {
        let mut acc = Self::one();
        let bits = 128 - e.leading_zeros();
        let mut i = bits;
        while i > 0 {
            i -= 1;
            acc = acc * acc;
            if (e >> i) & 1 == 1 {
                acc *= *self;
            }
        }
        acc
    }

    /// Multiplicative inverse by Fermat's little theorem: `x^(p-2)`.
    ///
    /// The exponent is the public constant `p - 2`, so the running time is
    /// independent of `x`. By convention `inv(0) = 0`.
    fn inv(&self) -> Self {
        self.pow_public(Self::MODULUS - 2)
    }
}

// ---------------------------------------------------------------------------
// GF(2^127 - 1)
// ---------------------------------------------------------------------------

/// The Mersenne prime 2^127 - 1.
pub const P127: u128 = (1u128 << 127) - 1;

/// An element of GF(2^127 - 1), always stored in canonical form `[0, p)`.
#[derive(Clone, Copy, Default, Zeroize)]
pub struct Fp127(u128);

impl fmt::Debug for Fp127 {
    // Never print field elements: they are secret in almost every context.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fp127(..)")
    }
}

/// Fold a 128-bit value: returns a value `<= 2^127` congruent to `x` mod p.
#[inline(always)]
const fn fold(x: u128) -> u128 {
    (x & P127) + (x >> 127)
}

/// Map `x <= 2^127` to its canonical representative in `[0, p)`.
///
/// `x - p` wraps (top bit set) exactly when `x < p`, which gives the mask.
#[inline(always)]
fn canon(x: u128) -> u128 {
    let t = x.wrapping_sub(P127);
    let keep_x = Choice::from_bit((t >> 127) as u8);
    ct_select_u128(keep_x, x, t)
}

impl Fp127 {
    /// Build from an integer that the caller guarantees is `< p`.
    ///
    /// Use [`Fp127::from_canonical`] for untrusted input.
    pub(crate) const fn from_raw(x: u128) -> Fp127 {
        Fp127(x)
    }

    /// Parse a canonical integer, rejecting `x >= p` so that every field
    /// element has exactly one encoding (no malleability).
    pub fn from_canonical(x: u128) -> Option<Fp127> {
        if x < P127 { Some(Fp127(x)) } else { None }
    }

    /// 16-byte little-endian canonical encoding.
    pub fn to_le_bytes(&self) -> [u8; 16] {
        self.0.to_le_bytes()
    }

    /// Parse the 16-byte little-endian canonical encoding.
    pub fn from_le_bytes(b: [u8; 16]) -> Option<Fp127> {
        Self::from_canonical(u128::from_le_bytes(b))
    }
}

impl Add for Fp127 {
    type Output = Fp127;
    #[inline]
    fn add(self, o: Fp127) -> Fp127 {
        // a, b < 2^127 so a + b < 2^128: no overflow.
        Fp127(canon(fold(self.0 + o.0)))
    }
}

impl Sub for Fp127 {
    type Output = Fp127;
    #[inline]
    fn sub(self, o: Fp127) -> Fp127 {
        // a - b = a + (p - b); p - b lies in [1, p].
        Fp127(canon(fold(self.0 + (P127 - o.0))))
    }
}

impl Neg for Fp127 {
    type Output = Fp127;
    #[inline]
    fn neg(self) -> Fp127 {
        Fp127(canon(P127 - self.0))
    }
}

impl Mul for Fp127 {
    type Output = Fp127;
    #[inline]
    fn mul(self, o: Fp127) -> Fp127 {
        // Schoolbook 2x2 limbs. a = a1*2^64 + a0 with a1 < 2^63, same for b.
        let (a0, a1) = (self.0 as u64 as u128, self.0 >> 64);
        let (b0, b1) = (o.0 as u64 as u128, o.0 >> 64);
        let ll = a0 * b0; // < 2^128
        let mid = a0 * b1 + a1 * b0; // each term < 2^127, sum < 2^128
        let hh = a1 * b1; // < 2^126
        let (lo, carry) = ll.overflowing_add(mid << 64);
        let hi = hh + (mid >> 64) + carry as u128; // product = hi*2^128 + lo, hi < 2^126
        // 2^128 = 2 (mod p), so product = lo + 2*hi (mod p).
        // fold(lo) <= 2^127 and 2*hi < 2^127, so the sum fits in 128 bits.
        let r = fold(lo) + (hi << 1);
        Fp127(canon(fold(r)))
    }
}

macro_rules! assign_ops {
    ($t:ty) => {
        impl AddAssign for $t {
            #[inline]
            fn add_assign(&mut self, o: $t) {
                *self = *self + o;
            }
        }
        impl SubAssign for $t {
            #[inline]
            fn sub_assign(&mut self, o: $t) {
                *self = *self - o;
            }
        }
        impl MulAssign for $t {
            #[inline]
            fn mul_assign(&mut self, o: $t) {
                *self = *self * o;
            }
        }
    };
}
assign_ops!(Fp127);

impl Field for Fp127 {
    const MODULUS: u128 = P127;
    const BYTES: usize = 16;

    fn zero() -> Self {
        Fp127(0)
    }
    fn one() -> Self {
        Fp127(1)
    }
    fn from_u64(x: u64) -> Self {
        Fp127(x as u128)
    }
    fn ct_eq(&self, other: &Self) -> Choice {
        // Both operands are canonical, so integer equality is field equality.
        ct_eq_u128(self.0, other.0)
    }
    fn ct_is_zero(&self) -> Choice {
        ct_is_nonzero_u128(self.0).not()
    }
    fn ct_select(c: Choice, a: &Self, b: &Self) -> Self {
        Fp127(ct_select_u128(c, a.0, b.0))
    }
    /// Normative sampling rule (mirrored by the Python reference):
    /// read 16 bytes, interpret little-endian, clear bit 127, reject if the
    /// result equals p. Output is exactly uniform on `[0, p)`.
    fn random<R: RandomSource + ?Sized>(rng: &mut R) -> Result<Self, Error> {
        loop {
            let mut b = [0u8; 16];
            rng.fill_bytes(&mut b)?;
            let x = u128::from_le_bytes(b) & P127;
            b.zeroize();
            // Rejection probability 2^-127. Branching on "x == p" reveals
            // only that a discarded candidate was p.
            if x != P127 {
                return Ok(Fp127(x));
            }
        }
    }
    fn to_u128(&self) -> u128 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Toy prime fields (lab only)
// ---------------------------------------------------------------------------

/// GF(P) for a small prime `P`. **Not constant time.** Lab use only.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Zeroize)]
pub struct Toy<const P: u64>(u64);

impl<const P: u64> Toy<P> {
    /// Reduce any integer into the field.
    pub const fn new(x: u64) -> Self {
        Toy(x % P)
    }
    /// The canonical representative.
    pub const fn value(&self) -> u64 {
        self.0
    }
    /// Iterate over every field element in order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..P).map(Toy)
    }
}

impl<const P: u64> fmt::Debug for Toy<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<const P: u64> Add for Toy<P> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Toy((self.0 + o.0) % P)
    }
}
impl<const P: u64> Sub for Toy<P> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Toy((self.0 + P - o.0) % P)
    }
}
impl<const P: u64> Neg for Toy<P> {
    type Output = Self;
    fn neg(self) -> Self {
        Toy((P - self.0) % P)
    }
}
impl<const P: u64> Mul for Toy<P> {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Toy(((self.0 as u128 * o.0 as u128) % P as u128) as u64)
    }
}
impl<const P: u64> AddAssign for Toy<P> {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}
impl<const P: u64> SubAssign for Toy<P> {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}
impl<const P: u64> MulAssign for Toy<P> {
    fn mul_assign(&mut self, o: Self) {
        *self = *self * o;
    }
}

impl<const P: u64> Field for Toy<P> {
    const MODULUS: u128 = P as u128;
    const BYTES: usize = 8;

    fn zero() -> Self {
        Toy(0)
    }
    fn one() -> Self {
        Toy(1 % P)
    }
    fn from_u64(x: u64) -> Self {
        Toy(x % P)
    }
    fn ct_eq(&self, other: &Self) -> Choice {
        Choice::from_bit((self.0 == other.0) as u8)
    }
    fn ct_is_zero(&self) -> Choice {
        Choice::from_bit((self.0 == 0) as u8)
    }
    fn ct_select(c: Choice, a: &Self, b: &Self) -> Self {
        if c.declassify() { *a } else { *b }
    }
    fn random<R: RandomSource + ?Sized>(rng: &mut R) -> Result<Self, Error> {
        let bound = P.next_power_of_two();
        loop {
            let mut b = [0u8; 8];
            rng.fill_bytes(&mut b)?;
            let x = u64::from_le_bytes(b) & (bound - 1);
            if x < P {
                return Ok(Toy(x));
            }
        }
    }
    fn to_u128(&self) -> u128 {
        self.0 as u128
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use proptest::prelude::*;

    fn big(x: u128) -> BigUint {
        BigUint::from(x)
    }
    fn p() -> BigUint {
        big(P127)
    }
    fn elem() -> impl Strategy<Value = u128> {
        // Bias towards edge cases: 0, 1, p-1, values near 2^64 and 2^126.
        prop_oneof![
            Just(0u128),
            Just(1u128),
            Just(P127 - 1),
            Just(P127 - 2),
            Just(1u128 << 64),
            Just((1u128 << 64) - 1),
            Just(1u128 << 126),
            0..P127,
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20_000))]

        #[test]
        fn mul_matches_bigint(a in elem(), b in elem()) {
            let r = (Fp127(a) * Fp127(b)).0;
            prop_assert_eq!(big(r), (big(a) * big(b)) % p());
        }

        #[test]
        fn add_sub_neg_match_bigint(a in elem(), b in elem()) {
            prop_assert_eq!(big((Fp127(a) + Fp127(b)).0), (big(a) + big(b)) % p());
            prop_assert_eq!(big((Fp127(a) - Fp127(b)).0), (big(a) + p() - big(b)) % p());
            prop_assert_eq!(big((-Fp127(a)).0), (p() - big(a)) % p());
        }

        #[test]
        fn inverse_is_inverse(a in 1..P127) {
            prop_assert_eq!((Fp127(a) * Fp127(a).inv()).0, 1);
        }

        #[test]
        fn results_are_canonical(a in elem(), b in elem()) {
            for r in [Fp127(a) + Fp127(b), Fp127(a) - Fp127(b), Fp127(a) * Fp127(b), -Fp127(a)] {
                prop_assert!(r.0 < P127);
            }
        }
    }

    #[test]
    fn inverse_of_zero_is_zero() {
        assert_eq!(Fp127::zero().inv().0, 0);
    }

    #[test]
    fn non_canonical_encodings_are_rejected() {
        assert!(Fp127::from_canonical(P127).is_none());
        assert!(Fp127::from_canonical(u128::MAX).is_none());
        assert!(Fp127::from_canonical(P127 - 1).is_some());
    }

    #[test]
    fn mersenne_identity() {
        // 2^127 = 1 mod p: (2^64)^2 * 2^-1 ... check 2^127 via repeated doubling.
        let two = Fp127::from_u64(2);
        assert_eq!(two.pow_public(127).0, 1);
    }

    #[test]
    fn toy_field_inverse() {
        type F = Toy<13>;
        for x in F::all().skip(1) {
            assert_eq!(x * x.inv(), F::one());
        }
    }
}
