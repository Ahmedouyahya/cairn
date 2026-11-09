# CAIRN

Threshold custody of master keys with information-theoretic security, using only linear algebra over the Mersenne field GF(2^127 - 1).

> **Status.** Research prototype written for the *Mathematics for Systems Security* hackathon (Cryptographer track). The design and its proofs are in [`paper/cairn.pdf`](paper/cairn.pdf). The code has not been independently audited; see [SECURITY.md](SECURITY.md) before using it for real keys.

CAIRN splits a master key into `n` shares so that any `t` recover it. Beyond plain secret sharing, it addresses five ways a shared key is exposed in practice. Each guarantee below is proved in the paper under the threat model of Section 2, and each has a reproducible check:

| Window | What goes wrong with textbook Shamir | CAIRN layer | Guarantee (proved in the [paper](paper/cairn.pdf)) | Evidence |
|---|---|---|---|---|
| **At rest** | stolen share files | Vandermonde sharing | `t-1` shares are independent of the key (perfect secrecy) | exhaustive enumeration, 0 violations in 77,525 dealings |
| **Over time** | a patient attacker collects `t` shares over months | zero-polynomial proactive refresh | a mobile adversary learns nothing | `attacks/mobile_adversary.py` |
| **In memory** | cold boot, swap, partial memory disclosure | split-state storage `sigma = <L, R>` | advantage `<= 5 * 2^-128` under 378 bits of arbitrary leakage from a share (32-byte key) | exact leakage tables |
| **During reconstruction** | timing and cache side channels | constant-time combine | no secret-dependent branch, address or latency | dudect `max|t| <= 2.1` (naive: 12,670), ctgrind 0 errors, assembly audit |
| **Under manipulation** | a cheating custodian steals the key (Tompa-Woll) | AMD armor | wrong key accepted with probability `<= (k+1)/p ~ 2^-125` | exhaustive and tight over 21.4 M secret/error pairs |

The scheme uses no block cipher, hash function or number-theoretic hardness assumption. All bounds are information-theoretic: they hold against computationally unbounded adversaries, and the only randomness assumption is the operating system's random number generator.

```mermaid
flowchart LR
    K[master key] --> E["L1 encode<br/>120-bit limbs"]
    E --> A["L2 AMD armor<br/>tau = x^(k+2) + sum s_i x^i"]
    A --> S["L3 share<br/>Sigma = C V (Vandermonde)"]
    S --> I["L4 store<br/>sigma = &lt;L, R&gt;, two memory regions"]
    I --> R["L5 refresh<br/>skew-symmetric Gibbs steps<br/>+ zero-polynomial deltas"]
    R --> C["L6 combine<br/>constant time, AMD check"]
    C --> K
```

## Quick start

```console
$ cargo build --release
$ alias cairn=target/release/cairn

# A fresh 32-byte key that is never printed and never touches the disk:
$ cairn split -t 3 -n 5 --generate 32 -o shares/
cairn: wrote 5 shares (any 3 recover the secret) to shares  set 3f9c2a...

# Proactive refresh: every share changes, the key does not, and old shares no longer combine with new ones.
$ cairn refresh shares/share-*.cairn
cairn: refreshed 5 share(s) to epoch 1

# Use the key without ever writing it down: it goes to a program over a pipe.
$ cairn combine shares/share-1.cairn shares/share-4.cairn shares/share-5.cairn --exec sha256sum
cairn: integrity verified; recovered with shares 1, 4, 5

# Custodian 1 cheats by altering one byte of her share.
$ printf '\x01' | dd of=shares/share-1.cairn bs=1 seek=700 conv=notrunc status=none
$ cairn combine shares/share-{1,2,3}.cairn --exec true
cairn: error: integrity check failed: shares were tampered with or are inconsistent

# With a fourth share present, the key is recovered and the cheater is named.
$ cairn combine shares/share-{1,2,3,4}.cairn --exec true
cairn: integrity verified; recovered with shares 2, 3, 4
cairn: WARNING inconsistent share(s): 1 (tampered, forged or stale)
```

Other commands: `cairn refresh --deltas-out DIR` for custodians who are not present, `cairn apply-delta`, `cairn rerandomize` (local re-randomisation of the storage), `cairn inspect`. The binary disables core dumps and `ptrace`, locks secret memory, writes shares with mode `0600`, never overwrites, and refuses to print a key to a terminal.

## The mathematics in brief

* **Field.** `p = 2^127 - 1`. Since `2^127 = 1 (mod p)`, reduction is a shift and an add; inversion is `x^(p-2)` with a public exponent. No division instruction is ever executed on secret data.
* **AMD armor** (Cramer et al. 2008). `a = (s_1..s_k, x, tau)` with `tau = x^(k+2) + sum s_i x^i`. Any error the cheaters add is independent of `x`, and a non-zero error turns acceptance into a non-zero polynomial equation of degree at most `k+1` in `x`.
* **Sharing.** `Sigma = C V` with `V` Vandermonde. Any `t-1` columns of `V` (without the constant row) have full rank, so the shares are uniform whatever the key.
* **Split-state storage.** Each share coordinate is stored as `(L, R)` with `<L, R> = sigma`. By Lindsey's lemma the inner product is a two-source extractor; our self-contained proof gives advantage `<= (1 + 1/p) 2^((lambda_L + lambda_R)/2) p^(-(ell-3)/2)`.
* **Refresh.** For a skew-symmetric `A`, `<A R, R> = 0`, and `A -> A R` is onto `R^perp`. So `L += A R; R += B L; L += C R` is a sequence of exact Gibbs steps that never computes `sigma` and never inverts anything.
* **Why three refresh steps.** The natural two-step refresh `L += A R; R += B L` is insecure: the new `L` still satisfies `<L_new, R_old> = sigma`, so reading one half before a refresh and the other half after it reveals the value. The third step brings the advantage to exactly `p^-(ell-1)` (Lemma 10 in the paper; the lab reproduces the value exactly at small parameters).

## Reproducing the results

```console
$ make verify      # everything below, then rebuilds the paper from the fresh results
```

| Check | Command | Result |
|---|---|---|
| Unit, property and tamper tests | `make test` | random edits of share files never yield a wrong key |
| Independent reference implementation | `make kat` | Python spec and Rust produce **byte-identical** shares, before and after refresh |
| Exact perfect secrecy | `cairn-lab secrecy` | 0 violations, every dealing enumerated |
| Exhaustive AMD bound | `cairn-lab amd` | worst case equals the bound `(k+1)/p` exactly (tight) |
| Cross-epoch refresh leakage | `cairn-lab refresh` | 2-step: TV = 1 (break); 3-step: TV = `p^-(ell-1)` exactly |
| Split-state leakage | `cairn-lab leakage` | one half alone: 0; both halves fully read: 1 |
| Timing (dudect) | `cairn-lab dudect` | CAIRN `max|t|` <= 2.1; textbook baseline up to 12,670 |
| ctgrind (Valgrind) | `make ctgrind` | CAIRN: 0 secret-dependent branches or addresses; baseline: 2,289 errors (positive control) |
| Assembly audit | `make asm` | field ops: 0 conditional jumps, 0 divisions |
| Undefined behaviour | `make miri` | 35/35 core tests pass under Miri |
| Attacks | `make attacks` | Tompa-Woll steals the key from plain Shamir, fails on CAIRN; mobile adversary fails |

Numbers above come from `results/*.json`; the paper's tables and figures are generated from the same files.

## Repository

```
crates/cairn-core   no_std, unsafe-free Rust: field, encoding, AMD, sharing, storage, refresh, combine, format
crates/cairn-cli    the `cairn` tool; its hardening module holds the only unsafe code (six libc calls)
crates/cairn-lab    exhaustive proofs, leakage analysis, dudect, ctgrind, benchmarks
crates/cairn-naive  deliberately vulnerable textbook baseline (attack target only)
reference/          executable specification in Python + known-answer vector generator
attacks/            Tompa-Woll and mobile-adversary demonstrations against the real binary
paper/              LaTeX paper, figures and tables generated from results/
tools/              ctgrind, assembly audit, house-style checks
```

## Threat model and limitations

CAIRN trusts the dealer while it deals and the combiner while it combines (the combiner outputs the key, by definition). It requires the combiner to know which custodian holds which share index (a roster); Proposition 11 in the paper shows that no scheme without a secret at the combiner can avoid an assumption of this kind. The leakage theorem covers storage; the computation that joins both halves is protected by constant-time code and measurement rather than by a leakage theorem. We did not measure power or electromagnetic emanations on hardware. The multi-epoch extension of the refresh analysis is open. Full list in the paper, Section 8.

## License

MIT. See [LICENSE](LICENSE).
