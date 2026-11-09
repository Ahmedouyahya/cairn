//! Experiment 2: the AMD bound, checked over every secret and every error.
//!
//! For each secret vector `s` in `F^k` and each non-zero error `D` in
//! `F^(k+2)` we count the AMD points `x` for which the tampered codeword
//! `E_x(s) + D` still verifies (library `amd::verify`). The theorem says the
//! count never exceeds `k + 1`. We report the worst case and how often it is
//! attained, which shows the bound is tight.

use cairn_core::Toy;
use cairn_core::amd::{tag, verify};
use serde_json::{Value, json};

use crate::common::{for_each_tuple, report};

fn check<const P: u64>(k: usize) -> Value {
    let p = P;
    let mut worst = 0u64;
    let mut attained = 0u64;
    let mut pairs = 0u64;
    let mut buf = vec![Toy::<P>::new(0); k + 2];
    for_each_tuple(p, k, |s| {
        let s: Vec<Toy<P>> = s.iter().map(|&v| Toy::new(v)).collect();
        let codewords: Vec<Vec<Toy<P>>> = (0..p)
            .map(|x| {
                let x = Toy::new(x);
                let mut c = s.clone();
                c.push(x);
                c.push(tag(&s, x));
                c
            })
            .collect();
        for_each_tuple(p, k + 2, |d| {
            if d.iter().all(|&v| v == 0) {
                return;
            }
            pairs += 1;
            let mut passes = 0u64;
            for c in &codewords {
                for i in 0..k + 2 {
                    buf[i] = c[i] + Toy::new(d[i]);
                }
                if verify(&buf).declassify() {
                    passes += 1;
                }
            }
            if passes > worst {
                worst = passes;
                attained = 0;
            }
            if passes == worst {
                attained += 1;
            }
        });
    });
    let bound = k as u64 + 1;
    json!({
        "p": p, "k": k,
        "secret_error_pairs": pairs,
        "worst_case_passing_x": worst,
        "theorem_bound_k_plus_1": bound,
        "worst_case_probability": worst as f64 / p as f64,
        "bound_probability": bound as f64 / p as f64,
        "pairs_attaining_worst_case": attained,
        "tight": worst == bound,
        "pass": worst <= bound,
    })
}

pub fn run() -> bool {
    let rows = vec![
        check::<31>(1),
        check::<13>(1),
        check::<13>(2),
        check::<7>(2),
        check::<7>(3),
        check::<11>(2),
    ];
    let pass = rows.iter().all(|r| r["pass"] == true);
    for r in &rows {
        println!(
            "    p={:<3} k={}  pairs={:<10} worst={}/{}  bound={}/{}  tight={}",
            r["p"],
            r["k"],
            r["secret_error_pairs"],
            r["worst_case_passing_x"],
            r["p"],
            r["theorem_bound_k_plus_1"],
            r["p"],
            r["tight"]
        );
    }
    report("amd", pass, json!(rows))
}
