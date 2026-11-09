//! Experiment 1: exact perfect secrecy and exact recoverability of L3.
//!
//! For every secret `a`, every coefficient vector `r` and every set `T` of
//! `t - 1` share indices, we tabulate the joint distribution of the shares in
//! `T`. Perfect secrecy holds iff that distribution is the same for every
//! `a`. We also check that every `t`-subset recovers `a` through the library's
//! Lagrange interpolation. Nothing is sampled: every case is enumerated.

use cairn_core::poly::{eval, lagrange_at_zero};
use cairn_core::{Field, Toy};
use serde_json::{Value, json};

use crate::common::{for_each_tuple, key, report, subsets};

fn check<const P: u64>(t: usize, n: u8) -> Value {
    type F<const Q: u64> = Toy<Q>;
    let p = P;
    let small = subsets(n, t - 1);
    let full = subsets(n, t);
    // hist[T][a] = histogram (as a dense vector) of share tuples on T.
    let width = p.pow((t - 1) as u32) as usize;
    let mut hist = vec![vec![vec![0u32; width]; p as usize]; small.len()];
    let mut recover_failures = 0u64;
    let mut cases = 0u64;
    let lambdas: Vec<Vec<F<P>>> = full.iter().map(|s| lagrange_at_zero(s).unwrap()).collect();

    for a in 0..p {
        for_each_tuple(p, t - 1, |r| {
            cases += 1;
            let mut coeffs = vec![F::<P>::new(a)];
            coeffs.extend(r.iter().map(|&x| F::<P>::new(x)));
            let shares: Vec<F<P>> =
                (1..=n).map(|j| eval(&coeffs, F::<P>::from_u64(j as u64))).collect();
            for (ti, set) in small.iter().enumerate() {
                let k = key(set.iter().map(|&j| shares[j as usize - 1].value()), p) as usize;
                hist[ti][a as usize][k] += 1;
            }
            for (si, set) in full.iter().enumerate() {
                let mut acc = F::<P>::zero();
                for (lam, &j) in lambdas[si].iter().zip(set) {
                    acc += *lam * shares[j as usize - 1];
                }
                if acc.value() != a {
                    recover_failures += 1;
                }
            }
        });
    }

    // Perfect secrecy: identical histograms across all secrets, for every T.
    let mut secrecy_violations = 0u64;
    for per_set in &hist {
        for h in per_set.iter().skip(1) {
            if h != &per_set[0] {
                secrecy_violations += 1;
            }
        }
    }
    // Stronger: each tuple appears exactly once (the map r -> shares is a bijection).
    let bijective = hist.iter().all(|ps| ps.iter().all(|h| h.iter().all(|&c| c == 1)));
    json!({
        "p": p, "t": t, "n": n,
        "dealings_enumerated": cases,
        "unauthorised_sets_checked": small.len(),
        "authorised_sets_checked": full.len(),
        "secrecy_violations": secrecy_violations,
        "every_t_minus_1_view_is_a_bijection_of_the_randomness": bijective,
        "recovery_failures": recover_failures,
        "pass": secrecy_violations == 0 && recover_failures == 0 && bijective,
    })
}

pub fn run() -> bool {
    let rows = vec![
        check::<13>(2, 4),
        check::<13>(3, 5),
        check::<13>(4, 6),
        check::<31>(3, 4),
        check::<7>(5, 6),
    ];
    let pass = rows.iter().all(|r| r["pass"] == true);
    for r in &rows {
        println!(
            "    p={:<3} t={} n={}  dealings={:<8} violations={} recovery_failures={}",
            r["p"],
            r["t"],
            r["n"],
            r["dealings_enumerated"],
            r["secrecy_violations"],
            r["recovery_failures"]
        );
    }
    report("secrecy", pass, json!(rows))
}
