## Inspiration

Shamir secret sharing is perfect *as mathematics*: any \(t\) of \(n\) shares recover a master key, any \(t-1\) reveal nothing. Deployed, it still leaks through five windows:

- **W1, at rest:** share files get copied.
- **W2, over time:** a patient attacker steals one share a month until she holds \(t\) of the same polynomial.
- **W3, in memory:** cold boot, swap, core dumps, partial memory disclosure.
- **W4, during reconstruction:** variable-time arithmetic leaks through timing and cache channels.
- **W5, under manipulation:** one cheating custodian submits a modified share; everyone else recovers a wrong key while she computes the right one (Tompa and Woll, 1988).

We wanted one algorithm that addresses all five, using only number theory and linear algebra, with **no computational assumption at all**: no AES, no hash, no discrete logarithm, no pre-existing cipher. Every guarantee had to be a theorem with an explicit bound, and every theorem had to be checked by a computation anyone can rerun.

## What it does

**CAIRN** keeps a master key under \(t\)-of-\(n\) custody. It works over the Mersenne field

$$\mathbb{F}_p,\qquad p = 2^{127}-1,$$

where \(2^{127}\equiv 1 \pmod p\) turns reduction into a shift and an add, and inversion is \(x^{-1}=x^{p-2}\) with a *public* exponent, so no division ever touches a secret.

### L1. Encoding
A secret of \(m\) bytes (UTF-8 for text) is length-prefixed, zero-padded and cut into 120-bit limbs \(s_1,\dots,s_k < 2^{120} < p\). The map is injective and decoding rejects anything that is not an exact encoding. A 32-byte key gives \(k=3\).

### L2. Algebraic manipulation detection (closes W5)
Draw \(x \in \mathbb{F}_p\) uniformly and armor the secret as

$$a = (s_1,\dots,s_k,\,x,\,\tau),\qquad \tau = f(x,s) = x^{k+2} + \sum_{i=1}^{k} s_i\,x^i .$$

**Lemma (AMD bound).** For every \(s\) and every non-zero error \(\Delta=(d,e,g)\) chosen independently of \(x\):

$$\Pr_x\big[\tau+g = f(x+e,\,s+d)\big] \;\le\; \frac{k+1}{p}\approx 2^{-125}.$$

*Proof.* If \(e\neq 0\), the difference \(f(x+e,s+d)-f(x,s)-g\) has leading term \((k+2)\,e\,x^{k+1}\neq 0\), so it has at most \(k+1\) roots. If \(e=0\) it is \(\sum_i d_i x^i - g\), of degree at most \(k\), and is identically zero only when \(\Delta=0\). \(\square\)

Because sharing is linear, cheaters can only *add* an error, and their view is independent of \(x\): Tompa-Woll cheating becomes a rejection, with no key output. With one extra share, the cheater is named.

### L3. Vandermonde sharing (closes W1)
For each coordinate \(c\), draw \(f_c(X) = a_c + \sum_{m=1}^{t-1} r_{c,m}X^m\). In matrix form the share matrix is

$$\Sigma = C\,V,\qquad V = \big[\,j^m\,\big]_{0\le m<t,\ 1\le j\le n}.$$

**Theorem (perfect secrecy).** Any \(t-1\) shares are independent of the key. *Proof:* restricted to a set \(B\) of at most \(t-1\) indices, the random part is \(V_B r_c\) with \(V_B = \operatorname{diag}(j)\,W\), \(W\) a Vandermonde matrix of full row rank, and \(\operatorname{diag}(j)\) invertible because index 0 is forbidden. So the shares are uniform whatever \(a_c\). \(\square\)

### L4. Split-state inner-product storage (closes W3)
No share value \(\sigma\) is ever stored. Each one is kept as two vectors in two separate memory regions (and two separate file sections):

$$\sigma = \langle L, R\rangle = \sum_{i=1}^{\ell} L_i R_i,\qquad (L,R)\in\mathbb{F}_p^{\ell}\times\mathbb{F}_p^{\ell},\ \ \ell = 8.$$

**Theorem (split-state leakage).** For any adaptive leakage of at most \(\lambda_L\) bits about \(L\) and \(\lambda_R\) bits about \(R\), and any two values \(\sigma,\sigma'\):

$$\mathrm{TV}(\Lambda_\sigma,\Lambda_{\sigma'}) \;\le\; \Big(1+\tfrac1p\Big)\,2^{(\lambda_L+\lambda_R)/2}\;p^{-(\ell-3)/2}.$$

With \(\ell = 8\) this allows **378 bits of arbitrary leakage at advantage \(2^{-128}\)** per coordinate; for a whole share stored in two regions, the same 378-bit total budget gives advantage \(D\cdot 2^{-128}\) (\(D=5\) for a 32-byte key). Leaking one half alone, even entirely, is useless. *Proof idea:* Lindsey's lemma bounds every character sum \(|\mathbb{E}\,\omega^{a\langle X,Y\rangle}|\le\sqrt{p^\ell/(|\mathcal A||\mathcal B|)}\) on any leakage rectangle \(\mathcal A\times\mathcal B\); Parseval turns that into a statistical distance; Cauchy-Schwarz over at most \(2^{\lambda_L+\lambda_R}\) rectangles finishes. The full proof is in the paper.

### L5. Refresh by skew-symmetric matrices (closes W2 and W3 over time)
To re-randomize \((L,R)\) without ever computing \(\sigma\), we use a fact from linear algebra: if \(A^{\top}=-A\), then \(\langle AR,R\rangle = R^{\top}A^{\top}R = -R^{\top}AR\), so \(\langle AR, R\rangle = 0\). Moreover \(A\mapsto AR\) is onto \(R^{\perp}\) (take \(A = vu^{\top}-uv^{\top}\) with \(\langle u,R\rangle=1\)), so \(AR\) is *uniform* on \(R^\perp\). The refresh

$$L \leftarrow L + A R,\qquad R \leftarrow R + B L,\qquad L \leftarrow L + C R\qquad (A,B,C \text{ random skew-symmetric})$$

is a sequence of exact Gibbs-sampler steps: it uses no inversion and no branch.

Across custodians, a coordinator sends each custodian \(\delta_j = g(j)\) for a random \(g\) with \(g(0)=0\); adding it moves everyone to a fresh, independent sharing of the same key. **Theorem (mobile adversary):** an attacker who corrupts at most \(t-1\) custodians per epoch learns nothing, ever.

**We broke our own first design.** The natural *two*-step refresh \(L\leftarrow L+AR,\ R\leftarrow R+BL\) looks complete, but the new \(L\) still satisfies \(\langle L_{\text{new}}, R_{\text{old}}\rangle = \sigma\): one memory dump of \(R\) before a refresh and one of \(L\) after it reveal the value exactly. The third step fixes it.

**Lemma (cross-epoch leakage).** For the three-step refresh and all \(\sigma,\sigma'\neq 0\):

$$\mathrm{TV}\big((R_{\text{old}},L_{\text{new}})_\sigma,\,(R_{\text{old}},L_{\text{new}})_{\sigma'}\big) = p^{-(\ell-1)}\approx 2^{-889}.$$

Our lab measured the two-step design at TV = 1 (total break) and the three-step design at exactly \(p^{-(\ell-1)}\), to every printed digit.

### L6. Constant-time reconstruction (closes W4)
For a \(t\)-subset \(T\), with Lagrange coefficients \(\lambda_j = \prod_{m\in T\setminus\{j\}} \frac{m}{m-j}\) computed from public indices only:

$$\hat a_c = \sum_{i=1}^{\ell}\ \sum_{j\in T} (\lambda_j L_{j,c,i})\,R_{j,c,i}.$$

The coordinate loop is outside and the share loop inside, so no individual share is ever formed, even in a register. Then the AMD tag is checked in constant time. All arithmetic on secrets is fixed sequences of multiplications, additions and masks.

## How we built it

- **Rust core** (`no_std`, `#![forbid(unsafe_code)]`): field, encoding, AMD, sharing, split-state storage, refresh, combination and a strict file format. Every algorithm is generic over a `Field` trait, so the *same code* that protects real keys runs in toy fields for exhaustive proofs.
- **The `cairn` CLI** is the applied environment: `split --generate 32` creates a key that never touches disk, `combine --exec CMD` pipes the recovered key into another program, `refresh` (with remote deltas), `rerandomize`, `inspect`. It disables core dumps and `ptrace`, locks memory, writes shares with mode 0600 and never prints a key to a terminal.
- **An independent Python reference** (an executable specification). Rust and Python produce **byte-identical** share files, before and after refresh.
- **A lab** that checks every theorem by computation:
  - perfect secrecy: 0 violations over every dealing at small primes;
  - AMD: worst case equals \((k+1)/p\) exactly over 13.4 million secret/error pairs (the bound is tight);
  - refresh: exact agreement with the lemma;
  - leakage: exact advantage tables.
- **Side-channel evidence:**
  - a dudect-style fixed-versus-random Welch test: CAIRN stays at a max \(|t|\) of about 2.1 or less in every run (threshold 4.5), while a textbook baseline on the same harness reaches \(|t|\) of 1,000 to about 12,900;
  - ctgrind under Valgrind: 0 secret-dependent branches or addresses in CAIRN, 2,289 errors in the baseline (the positive control);
  - an assembly audit: field multiplication compiles to 72 instructions with **0 conditional jumps and 0 divisions**;
  - Miri for undefined behaviour.
- **Attack scripts** run against the real binary. Tompa-Woll steals the key from plain Shamir and fails against CAIRN; a mobile adversary fails across refreshes.
- **A LaTeX paper** whose tables and figures are generated from the lab's JSON output. `make verify` reproduces everything.

## Challenges we ran into

- **Our own refresh was broken.** Exact enumeration through the production code showed that the obvious two-step refresh leaks the stored value across one refresh. Correctness tests could never have found it, because the value *is* preserved. We proved the fix and the exact residual \(p^{-(\ell-1)}\).
- **A timing test is only as good as its harness.** Our first harness reported a false leak (\(|t|\approx 22\)) in CAIRN's multiplication. It came from a branch that chose the input just before the timed region. Moving to dudect's precomputed-input design removed it. We report this in the paper rather than hide it.
- **What keyless schemes cannot do.** We proved that no threshold scheme without a secret at the combiner can stop an attacker who submits shares under arbitrary indices (she can simply deal her own key). So CAIRN states the roster assumption explicitly instead of hoping it away.

## Accomplishments that we're proud of

- Information-theoretic guarantees for all five windows, each with an explicit bound and a full proof under a stated threat model.
- A new inversion-free refresh from skew-symmetric matrices, with an exact cross-epoch analysis.
- Every claim has a reproducible measurement, and the paper's numbers are generated, not typed.
- Byte-for-byte agreement between two independent implementations.

## What we learned

The gap between "mathematically secure" and "secure on a computer" is where real attacks live, and closing it rigorously took as much linear algebra as the scheme itself. Above all: exact analysis finds what testing cannot.

## What's next

- Extend the cross-refresh analysis to adversaries that alternate halves over many epochs.
- Publicly verifiable shares without computational assumptions.
- Power and electromagnetic measurements on embedded hardware.
- A distributed refresh protocol with authenticated, confidential delta channels.

**Paper, code and every result:** see the repository link. `make verify` reproduces all of it.
