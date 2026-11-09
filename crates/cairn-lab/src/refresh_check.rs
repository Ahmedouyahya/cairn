//! Experiment 3: cross-epoch leakage of the inner-product refresh.
//!
//! Threat: an attacker steals one half of the storage *before* a refresh and
//! the other half *after* it (for example a memory dump of the R region on
//! Monday and of the L region on Tuesday). We compute, exactly, how well the
//! pair (old half, new half) distinguishes any two stored values `sigma`.
//!
//! * `2-step` refresh `L += A R; R += B L` (the obvious design): the new `L`
//!   still satisfies `<L_new, R_old> = sigma`, so the value leaks completely.
//! * `3-step` refresh `L += A R; R += B L; L += C R` (CAIRN): distance about
//!   `p^-(ell-1)`.
//!
//! Method A drives the production `ipenc::skew_step` / `ipenc::refresh`
//! through every possible random skew-symmetric matrix. Method B uses the
//! fact (proved in the paper, and confirmed here by agreement with method A)
//! that each half-step adds a uniform vector of the orthogonal hyperplane,
//! which makes larger parameters tractable.

use std::collections::HashMap;

use cairn_core::ipenc::{inner, refresh, skew_step};
use cairn_core::{Field, Toy};
use serde_json::{Value, json};

use crate::common::{ScriptRng, for_each_tuple, key, report};

/// Every pair `(L, R)` with `<L, R> = sigma` and `L_1 != 0`, i.e. the exact
/// support of `ipenc::encode_into`, on which its output is uniform.
fn encodings<const P: u64>(ell: usize, sigma: u64) -> Vec<(Vec<Toy<P>>, Vec<Toy<P>>)> {
    let mut out = Vec::new();
    for_each_tuple(P, 2 * ell - 1, |d| {
        if d[0] == 0 {
            return;
        }
        let l: Vec<Toy<P>> = d[..ell].iter().map(|&x| Toy::new(x)).collect();
        let mut r: Vec<Toy<P>> = vec![Toy::new(0)];
        r.extend(d[ell..].iter().map(|&x| Toy::new(x)));
        let rest = inner(&l[1..], &r[1..]);
        r[0] = (Toy::new(sigma) - rest) * l[0].inv();
        out.push((l, r));
    });
    out
}

fn vkey<const P: u64>(v: &[Toy<P>]) -> u64 {
    key(v.iter().map(|x| x.value()), P)
}

/// Method A: enumerate every random skew-symmetric matrix through the library.
fn method_a<const P: u64>(ell: usize, steps: usize) -> (f64, f64, bool) {
    let draws = steps * ell * (ell - 1) / 2;
    let span = P.pow(ell as u32);
    let mut h_rl: Vec<HashMap<u64, u64>> = Vec::new();
    let mut h_lr: Vec<HashMap<u64, u64>> = Vec::new();
    let mut total = 0u64;
    let mut sigma_always_recoverable = true;
    for sigma in 0..P {
        let (mut a, mut b) = (HashMap::new(), HashMap::new());
        let mut count = 0u64;
        for (l0, r0) in encodings::<P>(ell, sigma) {
            for_each_tuple(P, draws, |digits| {
                let (mut l, mut r) = (l0.clone(), r0.clone());
                let mut rng = ScriptRng::new(digits);
                if steps == 3 {
                    refresh(&mut l, &mut r, &mut rng).unwrap();
                } else {
                    skew_step(&mut l, &r, &mut rng).unwrap();
                    skew_step(&mut r, &l, &mut rng).unwrap();
                }
                assert_eq!(rng.consumed(), draws);
                assert_eq!(inner(&l, &r).value(), sigma, "refresh must preserve the value");
                if inner(&l, &r0).value() != sigma {
                    sigma_always_recoverable = false;
                }
                *a.entry(vkey(&r0) * span + vkey(&l)).or_insert(0) += 1;
                *b.entry(vkey(&l0) * span + vkey(&r)).or_insert(0) += 1;
                count += 1;
            });
        }
        total = count;
        h_rl.push(a);
        h_lr.push(b);
    }
    (
        crate::common::max_pairwise_tv(&h_rl, total),
        crate::common::max_pairwise_tv(&h_lr, total),
        sigma_always_recoverable,
    )
}

/// Method B: each half-step adds a uniform element of the orthogonal hyperplane.
fn method_b<const P: u64>(ell: usize, steps: usize) -> (f64, f64, f64, f64) {
    let span = P.pow(ell as u32);
    // perp[w] = all v with <v, w> = 0 (for w = 0: only v = 0, as A * 0 = 0).
    let all: Vec<Vec<Toy<P>>> = {
        let mut v = Vec::new();
        for_each_tuple(P, ell, |d| v.push(d.iter().map(|&x| Toy::new(x)).collect()));
        // Position in `all` must equal `vkey`, which reads digits most-significant first.
        v.sort_by_key(|x: &Vec<Toy<P>>| vkey(x));
        v
    };
    let perp: Vec<Vec<usize>> = all
        .iter()
        .map(|w| {
            if w.iter().all(|x| x.value() == 0) {
                vec![0]
            } else {
                (0..all.len()).filter(|&i| inner(&all[i], w).value() == 0).collect()
            }
        })
        .collect();
    let idx = |v: &[Toy<P>]| vkey(v) as usize;
    let add = |a: &[Toy<P>], b: &[Toy<P>]| -> Vec<Toy<P>> {
        a.iter().zip(b).map(|(x, y)| *x + *y).collect()
    };

    let mut h_rl: Vec<HashMap<u64, f64>> = Vec::new();
    let mut h_lr: Vec<HashMap<u64, f64>> = Vec::new();
    for sigma in 0..P {
        let enc = encodings::<P>(ell, sigma);
        let w0 = 1.0 / enc.len() as f64;
        let (mut a, mut b) = (HashMap::new(), HashMap::new());
        for (l0, r0) in &enc {
            // Distribution over (L, R) after each half-step, starting from (l0, r0).
            let mut states: Vec<(Vec<Toy<P>>, Vec<Toy<P>>, f64)> =
                vec![(l0.clone(), r0.clone(), w0)];
            for step in 0..steps {
                let mut next = Vec::new();
                for (l, r, w) in states {
                    let update_left = step % 2 == 0;
                    let pivot = if update_left { &r } else { &l };
                    let choices = &perp[idx(pivot)];
                    let wc = w / choices.len() as f64;
                    for &c in choices {
                        if update_left {
                            next.push((add(&l, &all[c]), r.clone(), wc));
                        } else {
                            next.push((l.clone(), add(&r, &all[c]), wc));
                        }
                    }
                }
                states = next;
            }
            for (l, r, w) in states {
                *a.entry(vkey(r0) * span + vkey(&l)).or_insert(0.0) += w;
                *b.entry(vkey(l0) * span + vkey(&r)).or_insert(0.0) += w;
            }
        }
        h_rl.push(a);
        h_lr.push(b);
    }
    let tv = |x: &HashMap<u64, f64>, y: &HashMap<u64, f64>| -> f64 {
        let mut s = 0.0;
        for (k, v) in x {
            s += (v - y.get(k).copied().unwrap_or(0.0)).abs();
        }
        for (k, v) in y {
            if !x.contains_key(k) {
                s += v;
            }
        }
        s / 2.0
    };
    let mut worst = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..h_rl.len() {
        for j in i + 1..h_rl.len() {
            let (d1, d2) = (tv(&h_rl[i], &h_rl[j]), tv(&h_lr[i], &h_lr[j]));
            worst.0 = worst.0.max(d1);
            worst.1 = worst.1.max(d2);
            if i > 0 {
                // Both values non-zero.
                worst.2 = worst.2.max(d1);
                worst.3 = worst.3.max(d2);
            }
        }
    }
    worst
}

fn row_a<const P: u64>(ell: usize) -> Vec<Value> {
    [2usize, 3]
        .iter()
        .map(|&steps| {
            let (rl, lr, leak) = method_a::<P>(ell, steps);
            json!({
                "method": "A (library, every skew matrix)", "p": P, "ell": ell,
                "refresh": format!("{steps}-step"),
                "tv_old_R_new_L": rl, "tv_old_L_new_R": lr,
                "old_R_and_new_L_reveal_sigma_exactly": leak,
            })
        })
        .collect()
}

fn row_b<const P: u64>(ell: usize) -> Vec<Value> {
    [2usize, 3]
        .iter()
        .map(|&steps| {
            let (rl, lr, rl_nz, lr_nz) = method_b::<P>(ell, steps);
            json!({
                "method": "B (hyperplane kernel)", "p": P, "ell": ell,
                "refresh": format!("{steps}-step"),
                "tv_old_R_new_L": rl, "tv_old_L_new_R": lr,
                "tv_old_R_new_L_nonzero_values": rl_nz, "tv_old_L_new_R_nonzero_values": lr_nz,
                "p_pow_minus_ell_minus_1": (P as f64).powi(-(ell as i32 - 1)),
            })
        })
        .collect()
}

pub fn run() -> bool {
    let mut rows = Vec::new();
    rows.extend(row_a::<3>(3));
    rows.extend(row_a::<5>(2));
    rows.extend(row_a::<7>(2));
    rows.extend(row_b::<3>(3));
    rows.extend(row_b::<5>(2));
    rows.extend(row_b::<5>(3));
    rows.extend(row_b::<3>(4));
    for r in &rows {
        let nz = r["tv_old_R_new_L_nonzero_values"]
            .as_f64()
            .map(|v| {
                format!(
                    "  [sigma,sigma' != 0: {v:.6} vs p^-(ell-1) = {:.6}]",
                    r["p_pow_minus_ell_minus_1"].as_f64().unwrap()
                )
            })
            .unwrap_or_default();
        println!(
            "    {:<34} p={} ell={} {}  TV(old R, new L)={:.6}  TV(old L, new R)={:.6}{nz}",
            r["method"].as_str().unwrap(),
            r["p"],
            r["ell"],
            r["refresh"].as_str().unwrap(),
            r["tv_old_R_new_L"].as_f64().unwrap(),
            r["tv_old_L_new_R"].as_f64().unwrap()
        );
    }
    // Claims checked (Lemma 10 in the paper):
    //  (i)   2-step refresh: old R and new L determine sigma (TV = 1);
    //  (ii)  3-step refresh, sigma != sigma' both non-zero: TV = p^-(ell-1) exactly;
    //  (iii) 3-step refresh, all pairs including sigma = 0: TV <= (p + 1) p^-(ell-1);
    //  (iv)  methods A and B agree wherever both are run.
    let mut pass = true;
    for r in &rows {
        let (p, ell) = (r["p"].as_f64().unwrap(), r["ell"].as_i64().unwrap() as i32);
        let unit = p.powi(-(ell - 1));
        let rl = r["tv_old_R_new_L"].as_f64().unwrap();
        let lr = r["tv_old_L_new_R"].as_f64().unwrap();
        if r["refresh"] == "2-step" {
            pass &= (rl - 1.0).abs() < 1e-9;
        } else {
            pass &= rl <= (p + 1.0) * unit + 1e-9 && lr <= (p + 1.0) * unit + 1e-9;
            if let Some(nz) = r["tv_old_R_new_L_nonzero_values"].as_f64() {
                pass &= (nz - unit).abs() < 1e-9;
                pass &= (r["tv_old_L_new_R_nonzero_values"].as_f64().unwrap() - unit).abs() < 1e-9;
            }
        }
    }
    for a in rows.iter().filter(|r| r["method"].as_str().unwrap().starts_with('A')) {
        for b in rows.iter().filter(|r| r["method"].as_str().unwrap().starts_with('B')) {
            if a["p"] == b["p"] && a["ell"] == b["ell"] && a["refresh"] == b["refresh"] {
                pass &= (a["tv_old_R_new_L"].as_f64().unwrap()
                    - b["tv_old_R_new_L"].as_f64().unwrap())
                .abs()
                    < 1e-9;
                pass &= (a["tv_old_L_new_R"].as_f64().unwrap()
                    - b["tv_old_L_new_R"].as_f64().unwrap())
                .abs()
                    < 1e-9;
            }
        }
    }
    report("refresh", pass, json!(rows))
}
