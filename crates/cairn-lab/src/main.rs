//! `cairn-lab`: every security claim of CAIRN, checked by computation.
//!
//! ```text
//! cargo run --release -p cairn-lab -- all        # everything below
//! cargo run --release -p cairn-lab -- secrecy    # exact perfect secrecy (L3)
//! cargo run --release -p cairn-lab -- amd        # exhaustive AMD bound (L2)
//! cargo run --release -p cairn-lab -- refresh    # cross-epoch leakage of refresh (L4/L5)
//! cargo run --release -p cairn-lab -- leakage    # bounded split-state leakage (L4)
//! cargo run --release -p cairn-lab -- dudect     # timing leakage, CAIRN vs naive baseline
//! cargo run --release -p cairn-lab -- bench      # performance
//! ```
//!
//! Each experiment writes `results/<name>.json` and the process exits
//! non-zero if any claimed bound is violated.

mod amd_check;
mod bench;
mod common;
mod ctgrind;
mod dudect;
mod leakage;
mod refresh_check;
mod secrecy;

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if which == "audit-touch" {
        audit::touch();
        return;
    }
    let all = which == "all";
    let mut ok = true;
    let mut ran = false;
    let mut run = |name: &str, f: fn() -> bool| {
        if all || which == name {
            println!("== {name}");
            let t = std::time::Instant::now();
            ok &= f();
            println!("   ({:.1} s)", t.elapsed().as_secs_f64());
            ran = true;
        }
    };
    run("secrecy", secrecy::run);
    run("amd", amd_check::run);
    run("refresh", refresh_check::run);
    run("leakage", leakage::run);
    run("dudect", dudect::run);
    run("bench", bench::run);
    // Run under Valgrind only, so not part of `all`.
    if which == "ctgrind-cairn" {
        ok &= ctgrind::run_cairn();
        ran = true;
    }
    if which == "ctgrind-naive" {
        ok &= ctgrind::run_naive();
        ran = true;
    }
    if !ran {
        eprintln!("unknown experiment `{which}`");
        std::process::exit(2);
    }
    std::process::exit(if ok { 0 } else { 1 });
}

/// Out-of-line copies of the hot paths, with stable symbol names, so that
/// `tools/asm_audit.sh` can disassemble exactly the code that runs.
pub mod audit {
    use cairn_core::{Field, Fp127, amd};

    #[unsafe(no_mangle)]
    #[inline(never)]
    pub fn cairn_audit_mul(a: &Fp127, b: &Fp127, out: &mut Fp127) {
        *out = *a * *b;
    }

    #[unsafe(no_mangle)]
    #[inline(never)]
    pub fn cairn_audit_add_sub(a: &Fp127, b: &Fp127, out: &mut Fp127) {
        *out = (*a + *b) - (-*b);
    }

    #[unsafe(no_mangle)]
    #[inline(never)]
    pub fn cairn_audit_amd_verify(armored: &[Fp127; 5]) -> u8 {
        amd::verify(armored).declassify() as u8
    }

    #[unsafe(no_mangle)]
    #[inline(never)]
    pub fn cairn_audit_select(c: u8, a: &Fp127, b: &Fp127, out: &mut Fp127) {
        *out = Fp127::ct_select(cairn_core::ct::Choice::from_bit(c), a, b);
    }

    /// Keep the symbols alive in the binary.
    pub fn touch() {
        let (a, b) = (Fp127::from_u64(3), Fp127::from_u64(5));
        let mut o = Fp127::zero();
        cairn_audit_mul(&a, &b, &mut o);
        cairn_audit_add_sub(&a, &b, &mut o);
        cairn_audit_select(1, &a, &b, &mut o);
        std::hint::black_box(cairn_audit_amd_verify(&[a, b, o, a, b]));
    }
}
