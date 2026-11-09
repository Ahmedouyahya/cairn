//! A deliberately **vulnerable** textbook implementation, used only as the
//! target of the attack demonstrations. Never use it for anything else.
//!
//! It is what a careful but side-channel-unaware engineer would write:
//!
//! * plain Shamir sharing over the same field GF(2^127 - 1), shares stored
//!   in the clear, no integrity protection (Tompa-Woll cheating works);
//! * modular multiplication by double-and-add with a branch on every bit of
//!   the multiplier, so its running time depends on the secret's bits;
//! * inversion by the extended Euclidean algorithm (data-dependent loop);
//! * an early-exit byte comparison.
//!
//! The lab measures the same operations in CAIRN and in this baseline with
//! the same harness, so the comparison is fair.

#![forbid(unsafe_code)]

/// The Mersenne prime 2^127 - 1.
pub const P: u128 = (1u128 << 127) - 1;

/// Modular addition (inputs below `P`).
pub fn add(a: u128, b: u128) -> u128 {
    let s = a + b;
    if s >= P { s - P } else { s }
}

/// Double-and-add multiplication: one conditional add per set bit of `b`.
pub fn mul(a: u128, b: u128) -> u128 {
    let mut acc = 0u128;
    let mut base = a;
    let mut e = b;
    while e != 0 {
        if e & 1 == 1 {
            acc = add(acc, base);
        }
        base = add(base, base);
        e >>= 1;
    }
    acc
}

/// Extended Euclid inverse (variable time).
pub fn inv(a: u128) -> u128 {
    let (mut r0, mut r1) = (P as i128, a as i128);
    let (mut t0, mut t1) = (0i128, 1i128);
    while r1 != 0 {
        let q = r0 / r1;
        (r0, r1) = (r1, r0 - q * r1);
        (t0, t1) = (t1, sub_mod(t0, mul(q as u128, t1.rem_euclid(P as i128) as u128) as i128));
    }
    t0.rem_euclid(P as i128) as u128
}

fn sub_mod(a: i128, b: i128) -> i128 {
    (a - b).rem_euclid(P as i128)
}

/// Plain Shamir: share `secret` (one field element per limb) `t`-of-`n`.
/// `coins` supplies the polynomial coefficients, `(t - 1)` per limb.
pub fn split(
    secret: &[u128],
    t: usize,
    n: u8,
    coins: &mut impl FnMut() -> u128,
) -> Vec<(u8, Vec<u128>)> {
    let polys: Vec<Vec<u128>> = secret
        .iter()
        .map(|&s| {
            let mut c = vec![s];
            c.extend((1..t).map(|_| coins() % P));
            c
        })
        .collect();
    (1..=n)
        .map(|x| {
            let vals = polys
                .iter()
                .map(|c| c.iter().rev().fold(0u128, |acc, &k| add(mul(acc, x as u128), k)))
                .collect();
            (x, vals)
        })
        .collect()
}

/// Plain Lagrange reconstruction at zero.
pub fn reconstruct(shares: &[(u8, Vec<u128>)]) -> Vec<u128> {
    let xs: Vec<u128> = shares.iter().map(|s| s.0 as u128).collect();
    let dim = shares[0].1.len();
    let mut out = vec![0u128; dim];
    for (j, (_, vals)) in shares.iter().enumerate() {
        let mut num = 1u128;
        let mut den = 1u128;
        for (m, &xm) in xs.iter().enumerate() {
            if m != j {
                num = mul(num, xm);
                den = mul(den, add(xm, P - xs[j]));
            }
        }
        let lambda = mul(num, inv(den));
        for (o, v) in out.iter_mut().zip(vals) {
            // The secret-dependent operand is the multiplier: time ~ popcount.
            *o = add(*o, mul(lambda, *v));
        }
    }
    out
}

/// Early-exit comparison.
pub fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (x, y) in a.iter().zip(b) {
        if x != y {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_correct() {
        assert_eq!(mul(3, 5), 15);
        assert_eq!(mul(P - 1, P - 1), 1);
        let x = 0x1234_5678_9abc_def0_1122_3344_5566_7788u128 % P;
        assert_eq!(mul(x, inv(x)), 1);
    }

    #[test]
    fn shamir_roundtrip() {
        let mut ctr = 7u128;
        let mut coins = || {
            ctr = ctr.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
            ctr
        };
        let shares = split(&[42, 4242], 3, 5, &mut coins);
        assert_eq!(reconstruct(&shares[1..4]), vec![42, 4242]);
    }
}
