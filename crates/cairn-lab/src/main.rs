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
