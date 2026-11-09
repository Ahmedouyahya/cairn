# CAIRN: video script (target 3:45)

**Setup before recording**

- `cargo build --release` in the repository root.
- A terminal at 110x32, font size 16 or larger, dark theme.
- The paper `paper/cairn.pdf` open in a viewer.
- Record the screen at 1080p (OBS, or KDE Spectacle's screen recorder). Speak slowly; captions help judges.
- Run `video/demo.sh` and press Enter between scenes. A dry run without pauses: `PAUSE=0 video/demo.sh`.

---

## 0:00 to 0:25, the problem (paper, Table 1 on screen)

> "Shamir secret sharing is perfect as mathematics. Deployed, a shared master key still leaks through five windows: stolen share files, a patient attacker who collects shares over months, memory disclosure, timing side channels, and a custodian who lies. CAIRN closes all five, using only linear algebra over one Mersenne prime, and without a single computational assumption: no AES, no hash, no discrete log."

## 0:25 to 1:05, the algorithm (paper, Figure 1, then the equations of Section 3)

> "We work in the field of integers modulo 2 to the 127 minus 1. Because 2 to the 127 is 1 in this field, reduction is a shift and an add, and nothing ever divides a secret.
> Layer 2 is an algebraic manipulation detection tag: tau equals x to the k plus 2, plus the sum of s_i x to the i. Any error a cheater adds turns acceptance into a polynomial equation of degree at most k plus 1, so it passes with probability 2 to the minus 125.
> Layer 3 shares the armored key with a Vandermonde matrix: any t minus 1 shares are uniformly random.
> Layer 4 never stores a share value. It stores two vectors L and R in separate memory regions whose inner product is the value. By Lindsey's lemma, an attacker can leak 378 bits from them and still gain essentially nothing: 2 to the minus 128 per coordinate.
> Layer 5 refreshes with random skew-symmetric matrices. A times R is always orthogonal to R, so the value never changes and never has to be computed."

## 1:05 to 1:40, the tool (demo scenes 1 and 2)

> "Here is the applied environment: a 32-byte key is generated inside the process and split three of five. It is never printed and never written to disk. Any three custodians can use it: combine pipes it straight into another program. Different subsets, same key."

## 1:40 to 2:15, the attacks (demo scenes 3 and 4)

> "Custodian 1 cheats by changing one byte. Against textbook Shamir, this Tompa-Woll attack lets her steal the key. Against CAIRN: integrity failure, zero bytes out. With a fourth share, the key is recovered and the cheater is named.
> Now a mobile attacker steals share 1, waits for a refresh, then steals shares 2 and 3. Different epochs: refused. Even if she forges the epoch header, the AMD check rejects the mixture."

## 2:15 to 2:50, side channels (demo scene 6, then `results/asm_audit.txt`)

> "Timing: we interleave fixed and random inputs and run Welch's t-test, the dudect method. Above 4.5 means leakage. A textbook implementation scores up to twelve thousand. CAIRN stays below 2.1 for multiplication, inversion and full reconstruction. The disassembly confirms it: field multiplication is 72 instructions with zero conditional jumps and zero divisions."

## 2:50 to 3:25, our own cryptanalysis (demo scene 7, then paper Figure 2)

> "The most important result is a break we found in our own first design. The natural refresh, two steps, preserves the value, so every correctness test passes. But the new L is still tied to the old R: one memory dump before a refresh and one after reveal the value exactly. Our lab measures a total-variation distance of 1. Adding a third step brings it to exactly p to the minus (ell minus 1), which we prove, and which the lab matches to every digit."

## 3:25 to 3:45, close (README evidence table, then `make verify` scrolling)

> "Every claim in CAIRN is a theorem with a proof, and every theorem has a computation that checks it: exhaustive enumeration, an independent reference implementation that matches byte for byte, timing tests, Valgrind and Miri. One command, make verify, reproduces all of it, including the paper. CAIRN: one stone says nothing; enough stones mark the way."

---

**Upload checklist**

- Title: "CAIRN: unconditional, leakage-resilient threshold custody of master keys".
- Visibility: Public or Unlisted; the link goes into the Devpost "Video demo link" field.
- Description: the repository link and the one-line summary.
