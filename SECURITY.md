# Security policy

CAIRN is a research prototype built for the *Mathematics for Systems Security* hackathon. It has not been independently audited. Do not rely on it for production keys without your own review.

## What we claim

The guarantees, their exact bounds and their assumptions are stated and proved in [`paper/cairn.pdf`](paper/cairn.pdf), Sections 3 to 5. In short:

* perfect secrecy against any `t-1` custodians;
* tamper detection with error at most `(k+1)/p` per evaluated subset;
* split-state leakage resilience (Theorem 7) and cross-refresh resilience (Lemma 10);
* security against a mobile adversary, given confidential refresh deltas;
* constant-time handling of secrets in the core, checked by dudect, ctgrind and an assembly audit.

## What we do not claim

See the paper's Limitations section. In particular:

* the dealer and the combiner are trusted while they run;
* robustness needs authenticated share indices (a roster);
* no power or electromagnetic analysis has been done on hardware.

## Reporting

Please open a GitHub issue for anything that contradicts a claim above. Include a reproducer if you can: the lab (`crates/cairn-lab`) and the Python reference (`reference/cairn_ref.py`) make most claims checkable in a few lines.
