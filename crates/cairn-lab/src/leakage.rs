//! Experiment 4: bounded leakage from both halves of the storage (L4).
//!
//! Part 1 (exact, toy field GF(5), ell = 4). We enumerate every encoding
//! `(L, R)` of every value `sigma` (the exact output distribution of
//! `ipenc::encode_into`) and compute the exact distinguishing advantage
//! `max_{sigma, sigma'} TV(leak | sigma, leak | sigma')` for leakage models
//! that a real side channel produces:
//!
//! * `coords(a, b)`: the attacker reads the first `a` coordinates of `L` and
//!   the first `b` coordinates of `R` in the clear (partial memory dump).
//! * `low_bits(b)`: the low `b` bits of every coordinate of both halves
//!   (a classic power / EM leakage model).
//! * `hamming`: the Hamming weight of every coordinate of both halves.
//!
//! Part 2 (production parameters). The leakage budget guaranteed by
//! Theorem 3 for arbitrary leakage functions:
//! `lambda_L + lambda_R <= (ell - 3) log2 p - 2 log2(1/eps)`.

use std::collections::HashMap;

use cairn_core::ipenc::inner;
use cairn_core::{Field, Toy};
use serde_json::{Value, json};

use crate::common::{for_each_tuple, key, max_pairwise_tv, report};

const P: u64 = 5;
const ELL: usize = 4;
type F = Toy<P>;

fn encodings(sigma: u64) -> Vec<(Vec<F>, Vec<F>)> {
    let mut out = Vec::new();
    for_each_tuple(P, 2 * ELL - 1, |d| {
        if d[0] == 0 {
            return;
        }
        let l: Vec<F> = d[..ELL].iter().map(|&x| F::new(x)).collect();
        let mut r = vec![F::new(0)];
        r.extend(d[ELL..].iter().map(|&x| F::new(x)));
        let rest = inner(&l[1..], &r[1..]);
        r[0] = (F::new(sigma) - rest) * l[0].inv();
        out.push((l, r));
    });
    out
}

type LeakFn = Box<dyn Fn(&[F], &[F]) -> u64>;

struct Model {
    name: String,
    bits: f64,
    leak: LeakFn,
}

fn models() -> Vec<Model> {
    let lp = (P as f64).log2();
    let mut out: Vec<Model> = Vec::new();
    for a in 0..=ELL {
        for b in 0..=ELL {
            out.push(Model {
                name: format!("coords({a},{b})"),
                bits: (a + b) as f64 * lp,
                leak: Box::new(move |l, r| {
                    key(l[..a].iter().chain(r[..b].iter()).map(|x| x.value()), P)
                }),
            });
        }
    }
    for bits in 1..=3u32 {
        out.push(Model {
            name: format!("low_bits({bits})"),
            bits: (2 * ELL) as f64 * (bits as f64).min(lp),
            leak: Box::new(move |l, r| {
                key(l.iter().chain(r).map(|x| x.value() & ((1 << bits) - 1)), 1 << bits)
            }),
        });
    }
    out.push(Model {
        name: "hamming".into(),
        bits: (2 * ELL) as f64 * 3f64.log2(),
        leak: Box::new(|l, r| key(l.iter().chain(r).map(|x| x.value().count_ones() as u64), 4)),
    });
    out
}

fn production_table() -> Vec<Value> {
    let lp = 127f64; // log2 p for p = 2^127 - 1, to within 2^-127
    [4u32, 6, 8, 12, 16]
        .iter()
        .map(|&ell| {
            let budget = |eps_bits: f64| ((ell as f64 - 3.0) * lp - 2.0 * eps_bits).max(0.0);
            json!({
                "ell": ell,
                "half_size_bits": ell as f64 * lp,
                "budget_bits_eps_2^-64": budget(64.0),
                "budget_bits_eps_2^-128": budget(128.0),
                "share_bytes_per_component": 2 * ell * 16,
            })
        })
        .collect()
}

pub fn run() -> bool {
    let encs: Vec<_> = (0..P).map(encodings).collect();
    let total = encs[0].len() as u64;
    let models = models();
    let mut rows = Vec::new();
    for m in &models {
        let hists: Vec<HashMap<u64, u64>> = encs
            .iter()
            .map(|e| {
                let mut h = HashMap::new();
                for (l, r) in e {
                    *h.entry((m.leak)(l, r)).or_insert(0) += 1;
                }
                h
            })
            .collect();
        let tv = max_pairwise_tv(&hists, total);
        rows.push(json!({ "model": m.name, "leaked_bits_upper_bound": m.bits, "advantage": tv }));
    }
    // Claims checked:
    //  - any amount of leakage from ONE half (coords(4,0)) gives advantage 0;
    //  - R alone (coords(0,4)) is within p^-(ell-1) of independent;
    //  - both halves in full (coords(4,4)) reveal sigma (advantage 1): the
    //    split-state assumption is necessary, not decorative.
    let get = |name: &str| {
        rows.iter().find(|r| r["model"] == name).unwrap()["advantage"].as_f64().unwrap()
    };
    let unit = (P as f64).powi(-(ELL as i32 - 1));
    let pass = get("coords(4,0)") == 0.0
        && get("coords(0,4)") <= unit + 1e-12
        && (get("coords(4,4)") - 1.0).abs() < 1e-12;
    for r in &rows {
        println!(
            "    {:<14} leaked <= {:>5.1} bits  advantage = {:.6}",
            r["model"].as_str().unwrap(),
            r["leaked_bits_upper_bound"].as_f64().unwrap(),
            r["advantage"].as_f64().unwrap()
        );
    }
    let table = production_table();
    for t in &table {
        println!(
            "    production ell={:<2}: arbitrary-leakage budget {:>6.0} bits at eps=2^-128 (half = {} bits)",
            t["ell"],
            t["budget_bits_eps_2^-128"].as_f64().unwrap(),
            t["half_size_bits"]
        );
    }
    report(
        "leakage",
        pass,
        json!({ "toy_field": { "p": P, "ell": ELL, "encodings_per_value": total, "models": rows },
                "production_budgets": table }),
    )
}
