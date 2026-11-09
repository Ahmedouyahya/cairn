//! `cairn-lab`: every security claim of CAIRN, checked by computation.
//!
//! ```text
//! cargo run --release -p cairn-lab -- all        # everything below
//! cargo run --release -p cairn-lab -- secrecy    # exact perfect secrecy (L3)
//! cargo run --release -p cairn-lab -- amd        # exhaustive AMD bound (L2)
//! cargo run --release -p cairn-lab -- refresh    # cross-epoch leakage of refresh (L4/L5)
//! cargo run --release -p cairn-lab -- leakage    # bounded split-state leakage (L4)
//! ```
//!
//! Each experiment writes `results/<name>.json` and the process exits
//! non-zero if any claimed bound is violated.

mod amd_check;
mod common;
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
    if !ran {
        eprintln!("unknown experiment `{which}`");
        std::process::exit(2);
    }
    std::process::exit(if ok { 0 } else { 1 });
}
