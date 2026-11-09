//! Experiment 7: performance (median wall-clock time over many runs).

use std::hint::black_box;
use std::time::Instant;

use cairn_core::scheme::{
    Share, apply_delta, combine_vector, refresh_all, refresh_deltas, refresh_local,
};
use cairn_core::testrng::SplitMix64;
use cairn_core::{Field, Fp127, Params, combine, split};
use serde_json::json;

use crate::common::report;

fn median_ns(runs: usize, mut f: impl FnMut()) -> f64 {
    for _ in 0..runs / 10 + 1 {
        f();
    }
    let mut v: Vec<f64> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_nanos() as f64
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn clone_share(s: &Share<Fp127>) -> Share<Fp127> {
    Share::from_parts(s.meta, s.left().to_vec(), s.right().to_vec()).unwrap()
}

pub fn run() -> bool {
    let mut rng = SplitMix64::new(1);
    let key = [7u8; 32];
    let params = Params::new(3, 5);
    let shares = split(&key, params, &mut rng).unwrap();
    let three: Vec<Share<Fp127>> = shares[..3].iter().map(clone_share).collect();
    let mut four: Vec<Share<Fp127>> = shares[..4].iter().map(clone_share).collect();
    four[0].halves_mut().1[0] += Fp127::one();
    let a = Fp127::from_u64(0x1234_5678);
    let b = Fp127::from_u64(0x9abc_def0);

    let mut rows = Vec::new();
    let mut add = |name: &str, ns: f64| {
        if ns < 1000.0 {
            println!("    {name:<48} {ns:>9.0} ns");
        } else {
            println!("    {name:<48} {:>9.1} us", ns / 1000.0);
        }
        rows.push(json!({ "operation": name, "median_ns": ns }));
    };
    add(
        "field multiplication",
        median_ns(200_000, || {
            black_box(black_box(a) * black_box(b));
        }),
    );
    add(
        "field inversion (Fermat, 127-bit exponent)",
        median_ns(20_000, || {
            black_box(black_box(a).inv());
        }),
    );
    add(
        "split 32-byte key, 3-of-5, ell=8",
        median_ns(2_000, || {
            black_box(split(&key, params, &mut rng).unwrap());
        }),
    );
    add(
        "combine 3 shares (verify + decode)",
        median_ns(5_000, || {
            black_box(combine(&three).unwrap());
        }),
    );
    add(
        "combine 4 shares, 1 cheater (identify)",
        median_ns(2_000, || {
            black_box(combine_vector(&four).unwrap());
        }),
    );
    let mut all: Vec<Share<Fp127>> = shares.iter().map(clone_share).collect();
    add(
        "proactive refresh of all 5 shares",
        median_ns(1_000, || {
            refresh_all(&mut all, &mut rng).unwrap();
        }),
    );
    let mut one = clone_share(&shares[0]);
    add(
        "local re-randomisation of one share",
        median_ns(2_000, || {
            refresh_local(&mut one, &mut rng).unwrap();
        }),
    );
    let mut tgt = clone_share(&shares[1]);
    add(
        "generate deltas + apply one delta",
        median_ns(2_000, || {
            let d = refresh_deltas::<Fp127, _>(&tgt.meta, &mut rng).unwrap();
            apply_delta(&mut tgt, &d[1], &mut rng).unwrap();
        }),
    );
    report("bench", true, json!({ "rows": rows, "note": "median of many runs, single core" }))
}
