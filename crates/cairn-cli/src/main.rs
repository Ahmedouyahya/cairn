//! `cairn`: keep a master key under t-of-n custody with the CAIRN scheme.
//!
//! ```text
//! cairn split   -t 3 -n 5 --generate 32 -o shares/      # new key, never shown, never on disk
//! cairn split   -t 3 -n 5 --in key.bin   -o shares/     # protect an existing key
//! cairn combine shares/share-1.cairn shares/share-4.cairn shares/share-5.cairn --exec CMD ARGS...
//! cairn refresh shares/*.cairn                          # proactive refresh, in place
//! cairn refresh shares/share-1.cairn --deltas-out d/    # refresh with remote custodians
//! cairn apply-delta share-2.cairn d/delta-2.cairn-delta # run by custodian 2
//! cairn rerandomize share-3.cairn                       # local re-randomisation of storage
//! cairn inspect shares/share-1.cairn                    # public metadata only
//! ```

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod hardening;

use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use cairn_core::format::{delta_from_bytes, delta_to_bytes, share_from_bytes, share_to_bytes};
use cairn_core::scheme::{apply_delta, refresh_deltas, refresh_local};
use cairn_core::{Error as CoreError, Fp127, Params, RandomSource, Share};
use clap::{Parser, Subcommand};
use zeroize::Zeroize;

use hardening::{LockedBytes, harden_process};

/// The operating system CSPRNG (`getrandom(2)`). Fails closed.
struct OsRng;

impl RandomSource for OsRng {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), CoreError> {
        getrandom::fill(dest).map_err(|_| CoreError::Rng)
    }
}

#[derive(Parser)]
#[command(name = "cairn", version, about = "Threshold custody of master keys (CAIRN scheme)")]
struct Cli {
    /// Print hardening status and extra details to stderr.
    #[arg(short, long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Split a secret into n share files, any t of which recover it.
    Split {
        /// Threshold t.
        #[arg(short = 't', long)]
        threshold: u8,
        /// Number of shares n.
        #[arg(short = 'n', long)]
        shares: u8,
        /// Read the secret from this file ("-" for stdin).
        #[arg(long = "in", conflicts_with = "generate")]
        input: Option<PathBuf>,
        /// Generate a fresh random secret of this many bytes instead. It is
        /// never printed and never written to disk: only the shares exist.
        #[arg(long)]
        generate: Option<usize>,
        /// Directory for the share files (created if missing, files never overwritten).
        #[arg(short, long)]
        out_dir: PathBuf,
        /// Inner-product dimension of the leakage-resilient storage.
        #[arg(long, default_value_t = 8)]
        ell: u8,
        /// Pad the secret to at least this many 15-byte limbs to hide its length.
        #[arg(long, default_value_t = 0)]
        pad_limbs: usize,
    },
    /// Recover the secret from t or more share files.
    Combine {
        /// Share files.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Write the secret to this file ("-" for stdout; refused on a terminal unless --force).
        #[arg(long, conflicts_with = "exec")]
        out: Option<PathBuf>,
        /// Run this command with the secret on its standard input. The secret
        /// never touches the disk, the environment or the command line.
        #[arg(long, num_args = 1.., allow_hyphen_values = true, value_name = "CMD")]
        exec: Option<Vec<String>>,
        /// Allow printing the secret to a terminal.
        #[arg(long)]
        force: bool,
    },
    /// Proactively refresh shares: every share changes, the secret does not,
    /// and shares from different epochs no longer combine.
    Refresh {
        /// Share files to refresh in place.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Also write deltas for custodians whose files are not present.
        #[arg(long)]
        deltas_out: Option<PathBuf>,
    },
    /// Apply a refresh delta received from the refresh coordinator.
    ApplyDelta { share: PathBuf, delta: PathBuf },
    /// Re-randomise a share's storage locally (value and epoch unchanged).
    Rerandomize { share: PathBuf },
    /// Show the public metadata of a share or delta file.
    Inspect { file: PathBuf },
}

type R<T> = Result<T, String>;

fn core(e: CoreError) -> String {
    e.to_string()
}

fn read_share(p: &Path) -> R<Share<Fp127>> {
    let mut bytes = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let s = share_from_bytes(&bytes).map_err(|e| format!("{}: {e}", p.display()));
    bytes.zeroize();
    s
}

/// Create a new file with mode 0600, refusing to overwrite.
fn write_new(path: &Path, data: &[u8]) -> R<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(data).and_then(|_| f.sync_all()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Replace a file atomically: write a 0600 temporary next to it, fsync, rename.
fn replace(path: &Path, data: &[u8]) -> R<()> {
    let tmp = path.with_extension("cairn.tmp");
    let _ = fs::remove_file(&tmp);
    write_new(&tmp, data)?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn read_secret(input: Option<PathBuf>, generate: Option<usize>) -> R<LockedBytes> {
    if let Some(len) = generate {
        if len == 0 || len > 65535 {
            return Err("--generate takes a length between 1 and 65535 bytes".into());
        }
        let mut buf = LockedBytes::with_capacity(len);
        let mut tmp = vec![0u8; len];
        getrandom::fill(&mut tmp).map_err(|_| "randomness source failed".to_string())?;
        buf.extend(&tmp);
        tmp.zeroize();
        return Ok(buf);
    }
    let path = input.ok_or("give --in FILE (or -) or --generate LEN")?;
    let mut buf = LockedBytes::with_capacity(65536 + 1);
    let mut reader: Box<dyn Read> = if path.as_os_str() == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?)
    };
    let mut chunk = [0u8; 4096];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if n > buf.spare_capacity() {
            chunk.zeroize();
            return Err("secret longer than 65535 bytes".into());
        }
        buf.extend(&chunk[..n]);
    }
    chunk.zeroize();
    Ok(buf)
}

fn cmd_split(
    t: u8,
    n: u8,
    input: Option<PathBuf>,
    generate: Option<usize>,
    out_dir: PathBuf,
    ell: u8,
    pad: usize,
) -> R<()> {
    if ell < 6 {
        eprintln!("warning: ell < 6 gives no arbitrary-leakage guarantee (see paper, Theorem 3)");
    }
    let secret = read_secret(input, generate)?;
    let params = Params { t, n, ell, min_limbs: pad };
    let shares = cairn_core::split(secret.as_slice(), params, &mut OsRng).map_err(core)?;
    drop(secret);

    fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let set = hex(&shares[0].meta.set_id);
    let mut roster = format!(
        "# CAIRN roster (public). Hand share-<i>.cairn to custodian <i> and record who holds it.\n\
         set_id {set}\nthreshold {t}\nshares {n}\nell {ell}\nlimbs {}\n",
        shares[0].meta.dim - 2
    );
    for s in &shares {
        let name = format!("share-{}.cairn", s.meta.index);
        write_new(&out_dir.join(&name), &share_to_bytes(s))?;
        roster.push_str(&format!("share {} {name} custodian: ________\n", s.meta.index));
    }
    write_new(&out_dir.join("roster.txt"), roster.as_bytes())?;
    eprintln!(
        "cairn: wrote {n} shares (any {t} recover the secret) to {}  set {}",
        out_dir.display(),
        &set[..16]
    );
    Ok(())
}

fn cmd_combine(
    files: Vec<PathBuf>,
    out: Option<PathBuf>,
    exec: Option<Vec<String>>,
    force: bool,
) -> R<()> {
    let shares: Vec<Share<Fp127>> = files.iter().map(|p| read_share(p)).collect::<R<_>>()?;
    let (secret, rec) = cairn_core::combine(&shares).map_err(core)?;
    drop(shares);
    let used: Vec<String> = rec.used.iter().map(|i| i.to_string()).collect();
    eprintln!("cairn: integrity verified; recovered with shares {}", used.join(", "));
    if !rec.inconsistent.is_empty() {
        let bad: Vec<String> = rec.inconsistent.iter().map(|i| i.to_string()).collect();
        eprintln!(
            "cairn: WARNING inconsistent share(s): {} (tampered, forged or stale)",
            bad.join(", ")
        );
    }
    let mut locked = LockedBytes::with_capacity(secret.len());
    locked.extend(&secret);
    drop(secret);

    if let Some(argv) = exec {
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", argv[0]))?;
        {
            let mut stdin = child.stdin.take().ok_or("no stdin")?;
            stdin.write_all(locked.as_slice()).map_err(|e| e.to_string())?;
        } // stdin closed here: the child sees EOF
        drop(locked);
        let status = child.wait().map_err(|e| e.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err(format!("command exited with {status}"))
        };
    }
    match out {
        Some(p) if p.as_os_str() != "-" => write_new(&p, locked.as_slice()),
        _ => {
            let stdout = io::stdout();
            if stdout.is_terminal() && !force {
                return Err("refusing to print the secret to a terminal (use --exec, --out FILE or --force)".into());
            }
            let mut lock = stdout.lock();
            lock.write_all(locked.as_slice()).and_then(|_| lock.flush()).map_err(|e| e.to_string())
        }
    }
}

fn cmd_refresh(files: Vec<PathBuf>, deltas_out: Option<PathBuf>) -> R<()> {
    let mut shares: Vec<Share<Fp127>> = files.iter().map(|p| read_share(p)).collect::<R<_>>()?;
    let meta = shares[0].meta;
    for s in &shares {
        if s.meta.set_id != meta.set_id || s.meta.epoch != meta.epoch {
            return Err("all files must belong to the same dealing and epoch".into());
        }
    }
    let present: Vec<u8> = shares.iter().map(|s| s.meta.index).collect();
    if present.len() < meta.n as usize && deltas_out.is_none() {
        return Err(format!(
            "only {} of {} shares given: pass --deltas-out DIR so the other custodians can apply their deltas",
            present.len(),
            meta.n
        ));
    }
    let deltas = refresh_deltas::<Fp127, _>(&meta, &mut OsRng).map_err(core)?;
    // Write the remote deltas first: if that fails, no local file has changed.
    if let Some(dir) = &deltas_out {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for d in deltas.iter().filter(|d| !present.contains(&d.meta.index)) {
            let name = format!("delta-{}-epoch{}.cairn-delta", d.meta.index, meta.epoch + 1);
            write_new(&dir.join(name), &delta_to_bytes(d))?;
        }
    }
    for (s, path) in shares.iter_mut().zip(&files) {
        apply_delta(s, &deltas[s.meta.index as usize - 1], &mut OsRng).map_err(core)?;
        replace(path, &share_to_bytes(s))?;
    }
    eprintln!(
        "cairn: refreshed {} share(s) to epoch {}{}",
        present.len(),
        meta.epoch + 1,
        if deltas_out.is_some() && present.len() < meta.n as usize {
            "; deliver each delta file to its custodian over a confidential channel, then delete it"
        } else {
            ""
        }
    );
    Ok(())
}

fn cmd_apply_delta(share: PathBuf, delta: PathBuf) -> R<()> {
    let mut s = read_share(&share)?;
    let mut bytes = fs::read(&delta).map_err(|e| format!("{}: {e}", delta.display()))?;
    let d = delta_from_bytes(&bytes).map_err(core);
    bytes.zeroize();
    let d = d?;
    apply_delta(&mut s, &d, &mut OsRng).map_err(core)?;
    replace(&share, &share_to_bytes(&s))?;
    eprintln!(
        "cairn: share {} moved to epoch {}; delete the delta file now",
        s.meta.index, s.meta.epoch
    );
    Ok(())
}

fn cmd_rerandomize(share: PathBuf) -> R<()> {
    let mut s = read_share(&share)?;
    refresh_local(&mut s, &mut OsRng).map_err(core)?;
    replace(&share, &share_to_bytes(&s))?;
    eprintln!("cairn: share {} storage re-randomised", s.meta.index);
    Ok(())
}

fn cmd_inspect(file: PathBuf) -> R<()> {
    let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let (kind, meta) = match share_from_bytes(&bytes) {
        Ok(s) => ("share", s.meta),
        Err(_) => ("refresh delta", delta_from_bytes(&bytes).map_err(core)?.meta),
    };
    println!("kind       {kind}");
    println!("set_id     {}", hex(&meta.set_id));
    println!("index      {} of {}", meta.index, meta.n);
    println!("threshold  {}", meta.t);
    println!("epoch      {}", meta.epoch);
    println!("limbs      {} (+2 AMD armor)", meta.dim - 2);
    println!(
        "ell        {} (split-state halves of {} bytes each)",
        meta.ell,
        meta.dim as usize * meta.ell as usize * 16
    );
    Ok(())
}

fn main() -> ExitCode {
    let h = harden_process();
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!(
            "cairn: hardening: no-dump={} no-core={} all-memory-locked={}",
            h.no_dump, h.no_core, h.all_memory_locked
        );
    }
    let result = match cli.cmd {
        Cmd::Split { threshold, shares, input, generate, out_dir, ell, pad_limbs } => {
            cmd_split(threshold, shares, input, generate, out_dir, ell, pad_limbs)
        }
        Cmd::Combine { files, out, exec, force } => cmd_combine(files, out, exec, force),
        Cmd::Refresh { files, deltas_out } => cmd_refresh(files, deltas_out),
        Cmd::ApplyDelta { share, delta } => cmd_apply_delta(share, delta),
        Cmd::Rerandomize { share } => cmd_rerandomize(share),
        Cmd::Inspect { file } => cmd_inspect(file),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cairn: error: {e}");
            ExitCode::FAILURE
        }
    }
}
