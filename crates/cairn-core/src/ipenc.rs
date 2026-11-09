//! L4: leakage-resilient inner-product storage.
//!
//! A field element `sigma` is never stored. Instead CAIRN stores a pair of
//! vectors `(L, R)` in `F^ell x F^ell` with `<L, R> = sigma`, kept in two
//! separate memory regions (and two separate sections of the share file).
//!
//! * A freshly encoded `L` is perfectly independent of `sigma`, and `R`
//!   alone is within statistical distance `p^-(ell-1)` of independent
//!   (it reveals something only in the event `R_2 = ... = R_ell = 0`).
//! * Bounded leakage from *both* halves is tolerated: with `lambda_L` and
//!   `lambda_R` bits of arbitrary leakage from each half, the advantage of
//!   any distinguisher is at most `2^((lambda_L + lambda_R)/2) * p^((3 - ell)/2)`
//!   (Theorem 7 in the paper, proved with the inner-product two-source
//!   extractor). For `ell = 8` and `p = 2^127 - 1` this allows 378 bits of
//!   leakage at advantage `2^-128`.
//!
//! ## Refresh by skew-symmetric matrices
//!
//! To re-randomize `(L, R)` without ever computing `sigma` we need vectors
//! orthogonal to `R` (or to `L`). For any skew-symmetric matrix `A`
//! (`A^T = -A`) we have `<A R, R> = R^T A^T R = -R^T A R`, hence
//! `<A R, R> = 0` in odd characteristic. For `R != 0` the map `A -> A R` is
//! onto the hyperplane `R^perp` (take `A = v w^T - w v^T` with `<w, R> = 1`),
//! so `A R` is *uniform* on `R^perp`. Each half-step below is therefore an
//! exact Gibbs-sampler step for the uniform distribution on
//! `{(L, R) : <L, R> = sigma}`:
//!
//! ```text
//! L'  = L  + A R      (A uniform skew-symmetric)
//! R'  = R  + B L'     (B uniform skew-symmetric)
//! L'' = L' + C R'     (C uniform skew-symmetric)      output (L'', R')
//! ```
//!
//! **Why three half-steps and not two.** With only the first two, the new
//! `L'` still satisfies `<L', R> = sigma` for the *old* `R`, so an attacker
//! who steals the old `R` half before the refresh and the new `L` half after
//! it computes `sigma` exactly. We found this in our own lab
//! (`cairn-lab refresh`). With the third half-step, any old half and any new
//! half are within statistical distance `p^-(ell-1)` of independent of
//! `sigma` (Lemma 10 in the paper), which the lab checks exactly at toy sizes.
//!
//! The refresh uses no inversion, no branch and no secret-dependent index.

use alloc::vec::Vec;
use zeroize::Zeroizing;

use crate::{Error, Field, RandomSource};

/// Inner product `<l, r>`.
pub fn inner<F: Field>(l: &[F], r: &[F]) -> F {
    let mut acc = F::zero();
    for (a, b) in l.iter().zip(r.iter()) {
        acc += *a * *b;
    }
    acc
}

/// Encode `sigma` as `(L, R)` with `<L, R> = sigma`, writing into `l` and `r`.
///
/// Normative randomness consumption: `L_1` (non-zero), then `L_2..L_ell`,
/// then `R_2..R_ell`. `R_1` is solved as `(sigma - sum_{i>=2} L_i R_i) / L_1`.
/// The inversion is of the random value `L_1`, by Fermat (constant time).
/// The output is uniform over all pairs with `<L, R> = sigma` and `L_1 != 0`.
pub fn encode_into<F: Field, R: RandomSource + ?Sized>(
    sigma: F,
    l: &mut [F],
    r: &mut [F],
    rng: &mut R,
) -> Result<(), Error> {
    let ell = l.len();
    debug_assert_eq!(ell, r.len());
    l[0] = F::random_nonzero(rng)?;
    for li in l[1..].iter_mut() {
        *li = F::random(rng)?;
    }
    for ri in r[1..].iter_mut() {
        *ri = F::random(rng)?;
    }
    let rest = inner(&l[1..], &r[1..]);
    r[0] = (sigma - rest) * l[0].inv();
    debug_assert!(ell >= 1);
    Ok(())
}

/// One Gibbs half-step: `v += M w` for a uniformly random skew-symmetric
/// `M`, drawn on the fly. Keeps `<v, w>` unchanged.
///
/// Normative randomness consumption: the strict upper triangle of `M`,
/// row by row (`m_{0,1}, m_{0,2}, ..., m_{ell-2, ell-1}`).
pub fn skew_step<F: Field, R: RandomSource + ?Sized>(
    v: &mut [F],
    w: &[F],
    rng: &mut R,
) -> Result<(), Error> {
    let ell = v.len();
    let mut delta: Zeroizing<Vec<F>> = Zeroizing::new(alloc::vec![F::zero(); ell]);
    for i in 0..ell {
        for j in (i + 1)..ell {
            let m = F::random(rng)?;
            // M[i][j] = m and M[j][i] = -m.
            delta[i] += m * w[j];
            delta[j] -= m * w[i];
        }
    }
    for (vi, di) in v.iter_mut().zip(delta.iter()) {
        *vi += *di;
    }
    Ok(())
}

/// Re-randomize `(L, R)` in place, keeping `<L, R>` unchanged.
///
/// Three Gibbs half-steps: `L`, then `R`, then `L` again (see module docs).
pub fn refresh<F: Field, R: RandomSource + ?Sized>(
    l: &mut [F],
    r: &mut [F],
    rng: &mut R,
) -> Result<(), Error> {
    skew_step(l, r, rng)?;
    skew_step(r, l, rng)?;
    skew_step(l, r, rng)?;
    Ok(())
}

/// Add a public-or-secret scalar `delta` to the encoded value in place:
/// afterwards `<L, R> = sigma + delta`.
///
/// Uses `R_1 += delta / L_1`. If `L_1` happens to be zero (probability about
/// `1/p` after a refresh) the pair is refreshed until it is not; the loop
/// condition reveals only that event.
pub fn add_scalar<F: Field, R: RandomSource + ?Sized>(
    l: &mut [F],
    r: &mut [F],
    delta: F,
    rng: &mut R,
) -> Result<(), Error> {
    while l[0].ct_is_zero().declassify() {
        refresh(l, r, rng)?;
    }
    r[0] += delta * l[0].inv();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Fp127, Toy};
    use crate::testrng::SplitMix64;

    #[test]
    fn encode_refresh_add_preserve_inner_product() {
        let mut rng = SplitMix64::new(9);
        for ell in [1usize, 2, 3, 8, 16] {
            let mut l = alloc::vec![Fp127::zero(); ell];
            let mut r = alloc::vec![Fp127::zero(); ell];
            let sigma = Fp127::from_u64(0xDEAD_BEEF);
            encode_into(sigma, &mut l, &mut r, &mut rng).unwrap();
            assert!(inner(&l, &r).ct_eq(&sigma).declassify());
            for _ in 0..10 {
                refresh(&mut l, &mut r, &mut rng).unwrap();
                assert!(inner(&l, &r).ct_eq(&sigma).declassify());
            }
            let d = Fp127::from_u64(77);
            add_scalar(&mut l, &mut r, d, &mut rng).unwrap();
            assert!(inner(&l, &r).ct_eq(&(sigma + d)).declassify());
        }
    }

    #[test]
    fn skew_product_is_orthogonal() {
        type F = Toy<31>;
        let mut rng = SplitMix64::new(3);
        for _ in 0..1000 {
            let w: Vec<F> = (0..5).map(|_| F::random(&mut rng).unwrap()).collect();
            let mut v = alloc::vec![F::zero(); 5];
            skew_step(&mut v, &w, &mut rng).unwrap();
            assert_eq!(inner(&v, &w), F::zero());
        }
    }

    #[test]
    fn refresh_changes_both_halves() {
        let mut rng = SplitMix64::new(5);
        let mut l = alloc::vec![Fp127::zero(); 8];
        let mut r = alloc::vec![Fp127::zero(); 8];
        encode_into(Fp127::from_u64(1), &mut l, &mut r, &mut rng).unwrap();
        let (l0, r0) = (l.clone(), r.clone());
        refresh(&mut l, &mut r, &mut rng).unwrap();
        assert!(l.iter().zip(&l0).any(|(a, b)| !a.ct_eq(b).declassify()));
        assert!(r.iter().zip(&r0).any(|(a, b)| !a.ct_eq(b).declassify()));
    }
}
