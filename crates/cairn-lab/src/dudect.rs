//! Experiment 5: timing leakage, fixed-versus-random Welch t-test.
//!
//! The methodology of dudect (Reparaz, Balasch, Verbauwhede, DATE 2017):
//! interleave measurements of the same operation on a *fixed* input (class
//! 0) and on *random* inputs (class 1) in random order, then test whether the
//! two timing distributions differ with Welch's t statistic. The test is run
//! on the raw samples and on several upper-percentile crops (which remove
//! interrupt noise). `|t| > 4.5` is the customary threshold (TVLA) for
//! "leakage detected"; dudect uses `|t| > 10` for "definitely not constant
//! time".
//!
//! Both implementations are measured by the same harness, on the same
//! machine, with inputs copied into the same working buffers before each
//! measurement so that cache placement cannot differ between the classes.

use std::hint::black_box;

use cairn_core::scheme::{Share, reconstruct_into, split_vector};
use cairn_core::testrng::SplitMix64;
use cairn_core::{Field, Fp127, Params, amd};
use serde_json::{Value, json};

use crate::common::report;

#[inline(always)]
fn cycles() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{_mm_lfence, _rdtsc};
        // SAFETY: lfence and rdtsc have no memory effects; x86_64 always has them.
        #[allow(unsafe_code)]
        unsafe {
            _mm_lfence();
            let t = _rdtsc();
            _mm_lfence();
            t
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        use std::time::Instant;
        thread_local!(static T0: Instant = Instant::now());
        T0.with(|t| t.elapsed().as_nanos() as u64)
    }
}

#[derive(Default, Clone, Copy)]
struct Welch {
    n: [f64; 2],
    mean: [f64; 2],
    m2: [f64; 2],
}

impl Welch {
    fn push(&mut self, class: usize, x: f64) {
        self.n[class] += 1.0;
        let d = x - self.mean[class];
        self.mean[class] += d / self.n[class];
        self.m2[class] += d * (x - self.mean[class]);
    }
    fn t(&self) -> f64 {
        let v0 = self.m2[0] / (self.n[0] - 1.0);
        let v1 = self.m2[1] / (self.n[1] - 1.0);
        (self.mean[0] - self.mean[1]) / (v0 / self.n[0] + v1 / self.n[1]).sqrt()
    }
}

/// Draw the random class sequence for a campaign (0 = fixed, 1 = random).
fn classes(samples: usize, seed: u64) -> Vec<usize> {
    let mut rng = SplitMix64::new(seed);
    (0..samples).map(|_| (rng.next_u64() & 1) as usize).collect()
}

/// Run `measure(i)` for every sample. Inputs are prepared in advance by the
/// caller, indexed by `i`, so the measured code path is byte-for-byte the
/// same for both classes (dudect's design). Returns the max |t| over the raw
/// data and percentile crops, plus per-crop values.
fn campaign(classes: &[usize], mut measure: impl FnMut(usize) -> u64) -> Value {
    let samples = classes.len();
    // Warm-up, discarded.
    for i in 0..samples / 20 {
        black_box(measure(i));
    }
    let data: Vec<(usize, u64)> = (0..samples).map(|i| (classes[i], measure(i))).collect();
    let mut sorted: Vec<u64> = data.iter().map(|d| d.1).collect();
    sorted.sort_unstable();
    let crops = [1.0f64, 0.99, 0.95, 0.90, 0.75, 0.50];
    let mut per_crop = Vec::new();
    let mut worst: f64 = 0.0;
    for &c in &crops {
        let cut = sorted[((sorted.len() as f64 - 1.0) * c) as usize];
        let mut w = Welch::default();
        for &(class, x) in &data {
            if x <= cut {
                w.push(class, x as f64);
            }
        }
        let t = w.t();
        worst = worst.max(t.abs());
        per_crop
            .push(json!({ "crop": c, "t": t, "mean_fixed": w.mean[0], "mean_random": w.mean[1] }));
    }
    let median = sorted[sorted.len() / 2];
    json!({ "samples": samples, "max_abs_t": worst, "median_cycles": median, "crops": per_crop })
}

fn rand_fp(rng: &mut SplitMix64) -> Fp127 {
    Fp127::random(rng).unwrap()
}

pub fn run_with(samples: usize) -> bool {
    let mut rng = SplitMix64::new(0xD0DEC7);
    let mut rows: Vec<(String, &'static str, Value)> = Vec::new();

    // --- Field multiplication: secret operand fixed vs random.
    let n = samples;
    let cls = classes(n, 1);
    let fixed = Fp127::from_u64(1);
    let other = rand_fp(&mut rng);
    let inputs: Vec<Fp127> =
        cls.iter().map(|&c| if c == 0 { fixed } else { rand_fp(&mut rng) }).collect();
    rows.push((
        "Fp127 multiplication".into(),
        "cairn",
        campaign(&cls, |i| {
            let x = black_box(inputs[i]);
            let t0 = cycles();
            let y = black_box(x * other);
            let t1 = cycles();
            black_box(y);
            t1 - t0
        }),
    ));
    let ninputs: Vec<u128> = inputs.iter().map(|x| x.to_u128()).collect();
    rows.push((
        "double-and-add multiplication".into(),
        "naive",
        campaign(&cls, |i| {
            let x = black_box(ninputs[i]);
            let t0 = cycles();
            let y = black_box(cairn_naive::mul(other.to_u128(), x));
            let t1 = cycles();
            black_box(y);
            t1 - t0
        }),
    ));

    // --- Inversion: secret input fixed vs random.
    let m = samples / 10;
    rows.push((
        "Fp127 inversion (Fermat)".into(),
        "cairn",
        campaign(&cls[..m], |i| {
            let x = black_box(inputs[i]);
            let t0 = cycles();
            let y = black_box(x.inv());
            let t1 = cycles();
            black_box(y);
            t1 - t0
        }),
    ));
    rows.push((
        "extended Euclid inversion".into(),
        "naive",
        campaign(&cls[..m], |i| {
            let x = black_box(ninputs[i]);
            let t0 = cycles();
            let y = black_box(cairn_naive::inv(x));
            let t1 = cycles();
            black_box(y);
            t1 - t0
        }),
    ));

    // --- Full reconstruction + integrity check, 3-of-5, 32-byte key.
    // Inputs: one fixed share set and 64 random ones; before each measurement
    // the chosen set is copied into the same working buffers.
    let key: Vec<Fp127> = (0..3).map(|_| rand_fp(&mut rng)).collect();
    let fixed_set = split_vector(&key, Params::new(3, 5), &mut rng).unwrap();
    let rand_sets: Vec<Vec<Share<Fp127>>> = (0..64)
        .map(|_| {
            let k: Vec<Fp127> = (0..3).map(|_| rand_fp(&mut rng)).collect();
            split_vector(&k, Params::new(3, 5), &mut rng).unwrap()
        })
        .collect();
    let sources: Vec<&Vec<Share<Fp127>>> = cls[..m]
        .iter()
        .enumerate()
        .map(|(i, &c)| if c == 0 { &fixed_set } else { &rand_sets[i % 64] })
        .collect();
    let mut work: Vec<Share<Fp127>> = split_vector(&key, Params::new(3, 5), &mut rng).unwrap();
    work.truncate(3);
    let mut out = vec![Fp127::zero(); 5];
    rows.push((
        "reconstruct + AMD verify (3-of-5, ell=8)".into(),
        "cairn",
        campaign(&cls[..m], |i| {
            for (w, s) in work.iter_mut().zip(sources[i].iter()) {
                let (l, r) = w.halves_mut();
                l.copy_from_slice(s.left());
                r.copy_from_slice(s.right());
            }
            let subset: Vec<&Share<Fp127>> = work.iter().collect();
            let t0 = cycles();
            reconstruct_into(black_box(&subset), &mut out).unwrap();
            let ok = black_box(amd::verify(&out));
            let t1 = cycles();
            assert!(ok.declassify());
            t1 - t0
        }),
    ));

    let mut coins_state = 99u128;
    let mut coins = || {
        coins_state =
            coins_state.wrapping_mul(0x9E37_79B9_7F4A_7C15_F39C_C060_5CED_C835).wrapping_add(1);
        coins_state >> 1
    };
    let nfixed = cairn_naive::split(&[1, 2, 3], 3, 5, &mut coins);
    let nrand: Vec<Vec<(u8, Vec<u128>)>> = (0..64)
        .map(|_| {
            let s = [coins() % cairn_naive::P, coins() % cairn_naive::P, coins() % cairn_naive::P];
            cairn_naive::split(&s, 3, 5, &mut coins)
        })
        .collect();
    let nsources: Vec<&Vec<(u8, Vec<u128>)>> = cls[..m]
        .iter()
        .enumerate()
        .map(|(i, &c)| if c == 0 { &nfixed } else { &nrand[i % 64] })
        .collect();
    let mut nwork: Vec<(u8, Vec<u128>)> = nfixed[..3].to_vec();
    rows.push((
        "plain Shamir reconstruct".into(),
        "naive",
        campaign(&cls[..m], |i| {
            for (w, s) in nwork.iter_mut().zip(nsources[i].iter()) {
                w.1.copy_from_slice(&s.1);
            }
            let t0 = cycles();
            let r = black_box(cairn_naive::reconstruct(black_box(&nwork)));
            let t1 = cycles();
            black_box(r);
            t1 - t0
        }),
    ));

    let mut pass = true;
    let mut table = Vec::new();
    for (name, imp, v) in rows {
        let t = v["max_abs_t"].as_f64().unwrap();
        let leak = t > 4.5;
        if imp == "cairn" {
            pass &= !leak;
        } else {
            pass &= t > 10.0; // the harness must be able to see a real leak
        }
        println!(
            "    {:<6} {:<42} median {:>7} cycles   max|t| = {:>9.2}  {}",
            imp,
            name,
            v["median_cycles"],
            t,
            if leak { "LEAKS" } else { "no leakage detected" }
        );
        table.push(json!({ "implementation": imp, "operation": name, "leak_detected": leak, "campaign": v }));
    }
    report("dudect", pass, json!({ "threshold": 4.5, "rows": table }))
}

pub fn run() -> bool {
    run_with(2_000_000)
}
