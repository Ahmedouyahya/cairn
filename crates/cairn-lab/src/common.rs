//! Shared helpers for the lab: scripted randomness, enumeration, reporting.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;

use cairn_core::{Error, RandomSource};
use serde_json::Value;

/// A "random" source that returns a fixed script of field digits.
///
/// Toy-field sampling reads 8 bytes and accepts the value if it is below
/// `P`, so feeding digit `d < P` makes the library draw exactly `d`. This
/// lets the lab drive the *production* algorithms through every possible
/// random choice.
pub struct ScriptRng<'a> {
    digits: &'a [u64],
    pos: usize,
}

impl<'a> ScriptRng<'a> {
    pub fn new(digits: &'a [u64]) -> Self {
        ScriptRng { digits, pos: 0 }
    }
    pub fn consumed(&self) -> usize {
        self.pos
    }
}

impl RandomSource for ScriptRng<'_> {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        assert_eq!(dest.len(), 8, "ScriptRng serves one toy field element per call");
        let d = *self.digits.get(self.pos).expect("script exhausted");
        dest.copy_from_slice(&d.to_le_bytes());
        self.pos += 1;
        Ok(())
    }
}

/// Visit every tuple in `{0..base}^len` in odometer order.
pub fn for_each_tuple(base: u64, len: usize, mut f: impl FnMut(&[u64])) {
    let mut t = vec![0u64; len];
    loop {
        f(&t);
        let mut i = 0;
        loop {
            if i == len {
                return;
            }
            t[i] += 1;
            if t[i] < base {
                break;
            }
            t[i] = 0;
            i += 1;
        }
    }
}

/// Encode a tuple of digits as one integer key.
pub fn key(digits: impl IntoIterator<Item = u64>, base: u64) -> u64 {
    digits.into_iter().fold(0, |acc, d| acc * base + d)
}

/// Total variation distance between two count histograms with equal totals.
pub fn tv_distance<K: Eq + Hash>(a: &HashMap<K, u64>, b: &HashMap<K, u64>, total: u64) -> f64 {
    let mut diff: u128 = 0;
    for (k, &x) in a {
        let y = b.get(k).copied().unwrap_or(0);
        diff += x.abs_diff(y) as u128;
    }
    for (k, &y) in b {
        if !a.contains_key(k) {
            diff += y as u128;
        }
    }
    diff as f64 / (2.0 * total as f64)
}

/// Maximum pairwise total variation distance over a family of histograms.
pub fn max_pairwise_tv<K: Eq + Hash>(hists: &[HashMap<K, u64>], total: u64) -> f64 {
    let mut worst: f64 = 0.0;
    for i in 0..hists.len() {
        for j in i + 1..hists.len() {
            worst = worst.max(tv_distance(&hists[i], &hists[j], total));
        }
    }
    worst
}

/// All `k`-subsets of `1..=n`.
pub fn subsets(n: u8, k: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut idx: Vec<u8> = (1..=k as u8).collect();
    if k == 0 {
        return vec![vec![]];
    }
    loop {
        out.push(idx.clone());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if idx[i] < n - (k - 1 - i) as u8 {
                idx[i] += 1;
                for j in i + 1..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
        }
    }
}

/// Directory where results are written (`<repo>/results`).
pub fn results_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../results");
    std::fs::create_dir_all(&dir).expect("create results dir");
    dir
}

/// Write a JSON report and print a one-line verdict.
pub fn report(name: &str, pass: bool, body: Value) -> bool {
    let path = results_dir().join(format!("{name}.json"));
    let doc = serde_json::json!({ "experiment": name, "pass": pass, "result": body });
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    println!("[{}] {name}  ->  results/{name}.json", if pass { "PASS" } else { "FAIL" });
    pass
}
