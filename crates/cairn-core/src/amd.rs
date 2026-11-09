//! L2: algebraic manipulation detection (AMD) armor.
//!
//! For a secret vector `s = (s_1, ..., s_k)` draw a uniform `x` and output
//!
//! ```text
//! E(s) = (s_1, ..., s_k, x, tau)   with   tau = f(x, s) = x^(k+2) + sum_i s_i x^i
//! ```
//!
//! **Theorem (AMD bound).** If `k + 2` is not divisible by the characteristic
//! `p`, then for every secret `s` and every non-zero error vector `D` chosen
//! independently of `x`, `Pr_x[ E(s) + D passes ] <= (k + 1) / p`.
//!
//! *Proof sketch.* Writing `D = (d, e, g)`, acceptance means
//! `f(x + e, s + d) - f(x, s) - g = 0`. If `e != 0` this is a polynomial in
//! `x` whose leading term is `(k+2) e x^(k+1) != 0`, so it has at most `k + 1`
//! roots. If `e = 0` it equals `sum_i d_i x^i - g`, of degree at most `k`, and
//! it is the zero polynomial only if `d = 0` and `g = 0`. The full proof is in
//! the paper; the lab (`cairn-lab exhaustive`) checks the bound over every `s`
//! and every `D` in small fields.
//!
//! The construction is due to Cramer, Dodis, Fehr, Padró and Wichs (2008).
//! Inside a linear secret sharing scheme, the shares of up to `t-1` cheaters
//! can only add an error that is independent of `x`, which is exactly the
//! hypothesis of the theorem. No verification key is stored anywhere.

use alloc::vec::Vec;
use zeroize::Zeroizing;

use crate::ct::Choice;
use crate::{Error, Field, RandomSource};

/// `f(x, s) = x^(k+2) + sum_{i=1..k} s_i x^i`, by Horner's rule.
///
/// Write `f = x * g` with `g = s_1 + x(s_2 + ... + x(s_k + x * x))`. The loop
/// runs exactly `k` times and performs one multiplication and one addition
/// per step, whatever the values.
pub fn tag<F: Field>(s: &[F], x: F) -> F {
    let mut acc = x;
    for si in s.iter().rev() {
        acc = *si + x * acc;
    }
    x * acc
}

/// Check that `k + 2` is not a multiple of the field characteristic.
fn check_dimension<F: Field>(k: usize) -> Result<(), Error> {
    if k == 0 {
        return Err(Error::InvalidParams("AMD armor needs at least one limb"));
    }
    if (k as u128 + 2) % F::MODULUS == 0 {
        return Err(Error::InvalidParams("k + 2 is divisible by the field characteristic"));
    }
    Ok(())
}

/// Armor a secret vector: returns `(s_1, ..., s_k, x, tau)`.
///
/// Normative randomness consumption: exactly one field element, `x`.
pub fn armor<F: Field, R: RandomSource + ?Sized>(
    s: &[F],
    rng: &mut R,
) -> Result<Zeroizing<Vec<F>>, Error> {
    check_dimension::<F>(s.len())?;
    let x = F::random(rng)?;
    let mut out = Zeroizing::new(Vec::with_capacity(s.len() + 2));
    out.extend_from_slice(s);
    out.push(x);
    out.push(tag(s, x));
    Ok(out)
}

/// Constant-time verification of an armored vector.
///
/// Returns a secret [`Choice`]; the caller decides when to declassify it.
pub fn verify<F: Field>(armored: &[F]) -> Choice {
    if armored.len() < 3 {
        return Choice::FALSE;
    }
    let k = armored.len() - 2;
    let (s, rest) = armored.split_at(k);
    tag(s, rest[0]).ct_eq(&rest[1])
}

/// The secret part `(s_1, ..., s_k)` of an armored vector.
pub fn payload<F: Field>(armored: &[F]) -> &[F] {
    &armored[..armored.len().saturating_sub(2)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Fp127, Toy};
    use crate::testrng::SplitMix64;

    #[test]
    fn tag_matches_definition() {
        type F = Toy<101>;
        let s = [F::new(3), F::new(7), F::new(11)];
        for x in F::all() {
            let mut expect = x.pow_public(5);
            for (i, si) in s.iter().enumerate() {
                expect += *si * x.pow_public(i as u128 + 1);
            }
            assert_eq!(tag(&s, x), expect);
        }
    }

    #[test]
    fn honest_armor_verifies_and_tamper_fails() {
        let mut rng = SplitMix64::new(7);
        let s: Vec<Fp127> = (0..3).map(|i| Fp127::from_u64(1000 + i)).collect();
        let mut a = armor(&s, &mut rng).unwrap();
        assert!(verify(&a).declassify());
        a[1] += Fp127::one();
        assert!(!verify(&a).declassify());
    }

    #[test]
    fn rejects_bad_dimension() {
        let mut rng = SplitMix64::new(1);
        // k + 2 = 7 is divisible by 7.
        let s = [Toy::<7>::new(1); 5];
        assert!(armor(&s, &mut rng).is_err());
        assert!(armor::<Fp127, _>(&[], &mut rng).is_err());
    }
}
