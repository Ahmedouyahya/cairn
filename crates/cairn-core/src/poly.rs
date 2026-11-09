//! L3: polynomial (Shamir-type) sharing in matrix form.
//!
//! For an armored vector `a` of dimension `D`, the dealer draws a coefficient
//! matrix `C` in `F^(D x t)` whose first column is `a` and whose other columns
//! are uniform. Share `j` is the column `C * v_j` where
//! `v_j = (1, j, j^2, ..., j^(t-1))` is a column of the Vandermonde matrix `V`:
//!
//! ```text
//! Sigma = C * V,   V = [ j^m ]_{m = 0..t-1, j = 1..n}
//! ```
//!
//! Any `t` columns of `V` form an invertible Vandermonde matrix
//! (determinant `prod_{i<j} (x_j - x_i) != 0`), which gives reconstruction.
//! Any `t-1` columns restricted to rows `1..t-1` form `diag(x_j) * W` with `W`
//! an invertible Vandermonde matrix, which gives perfect secrecy: the map from
//! the random columns of `C` to the `t-1` shares is a bijection for every `a`.
//!
//! Lagrange coefficients depend only on public share indices.

use alloc::vec::Vec;
use zeroize::Zeroizing;

use crate::{Error, Field, RandomSource};

/// Evaluate `c_0 + c_1 x + ... + c_{t-1} x^{t-1}` by Horner's rule.
pub fn eval<F: Field>(coeffs: &[F], x: F) -> F {
    let mut acc = F::zero();
    for c in coeffs.iter().rev() {
        acc = acc * x + *c;
    }
    acc
}

/// Draw the coefficient matrix for sharing `values` with threshold `t`.
///
/// Returns a row-major `D x t` matrix. Normative randomness consumption:
/// for each component `c` in order, `t - 1` field elements (coefficients of
/// `x^1 .. x^(t-1)`).
pub fn coefficients<F: Field, R: RandomSource + ?Sized>(
    values: &[F],
    t: usize,
    rng: &mut R,
) -> Result<Zeroizing<Vec<F>>, Error> {
    let mut c = Zeroizing::new(Vec::with_capacity(values.len() * t));
    for v in values {
        c.push(*v);
        for _ in 1..t {
            c.push(F::random(rng)?);
        }
    }
    Ok(c)
}

/// Evaluate every row of a row-major `D x t` coefficient matrix at `x`.
pub fn eval_rows<F: Field>(coeffs: &[F], t: usize, x: F) -> Zeroizing<Vec<F>> {
    Zeroizing::new(coeffs.chunks_exact(t).map(|row| eval(row, x)).collect())
}

/// Lagrange coefficients for interpolation at zero from the points `xs`.
///
/// `lambda_j = prod_{m != j} x_m / (x_m - x_j)`. All inputs are public.
/// Fails on repeated or zero points, which would make the system singular
/// or place a share at the secret's own evaluation point.
pub fn lagrange_at_zero<F: Field>(xs: &[u8]) -> Result<Vec<F>, Error> {
    for (i, a) in xs.iter().enumerate() {
        if *a == 0 {
            return Err(Error::InvalidParams("share index 0 is the secret itself"));
        }
        if xs[..i].contains(a) {
            return Err(Error::DuplicateIndex(*a));
        }
    }
    let pts: Vec<F> = xs.iter().map(|&x| F::from_u64(x as u64)).collect();
    let mut out = Vec::with_capacity(pts.len());
    for (j, xj) in pts.iter().enumerate() {
        let mut num = F::one();
        let mut den = F::one();
        for (m, xm) in pts.iter().enumerate() {
            if m != j {
                num *= *xm;
                den *= *xm - *xj;
            }
        }
        out.push(num * den.inv());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Fp127, Toy};
    use crate::testrng::SplitMix64;

    #[test]
    fn interpolation_recovers_constant_term() {
        let mut rng = SplitMix64::new(42);
        let secret = [Fp127::from_u64(123_456_789)];
        for t in 1..=6usize {
            let c = coefficients(&secret, t, &mut rng).unwrap();
            let xs: Vec<u8> = (1..=t as u8).map(|i| i * 3).collect();
            let lam: Vec<Fp127> = lagrange_at_zero(&xs).unwrap();
            let mut acc = Fp127::zero();
            for (x, l) in xs.iter().zip(&lam) {
                acc += *l * eval(&c, Fp127::from_u64(*x as u64));
            }
            assert!(acc.ct_eq(&secret[0]).declassify(), "t = {t}");
        }
    }

    #[test]
    fn rejects_degenerate_points() {
        assert!(lagrange_at_zero::<Toy<13>>(&[1, 2, 1]).is_err());
        assert!(lagrange_at_zero::<Toy<13>>(&[0, 2]).is_err());
    }
}
