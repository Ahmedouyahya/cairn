"""Generate known-answer test vectors from the Python reference.

The Rust test `crates/cairn-core/tests/kat.rs` replays each case with the
same SplitMix64 seed and must reproduce every share byte for byte, before
and after a proactive refresh.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cairn_ref import SplitMix64, combine, refresh_all, split  # noqa: E402

CASES = [
    dict(seed=1, t=2, n=3, ell=8, min_limbs=0, secret=bytes(range(32)).hex()),
    dict(seed=2, t=3, n=5, ell=8, min_limbs=0, secret=bytes(32).hex()),
    dict(seed=3, t=1, n=1, ell=8, min_limbs=0, secret=b"".hex()),
    dict(seed=4, t=5, n=7, ell=4, min_limbs=4, secret="clé maîtresse 🔑".encode().hex()),
    dict(seed=5, t=4, n=4, ell=2, min_limbs=0, secret=(b"\xff" * 61).hex()),
]

out = []
for case in CASES:
    rng = SplitMix64(case["seed"])
    secret = bytes.fromhex(case["secret"])
    shares = split(secret, case["t"], case["n"], rng, case["ell"], case["min_limbs"])
    first = [s.to_bytes().hex() for s in shares]
    refresh_all(shares, rng)
    assert combine(shares) == secret
    out.append({**case, "shares": first, "refreshed": [s.to_bytes().hex() for s in shares]})

path = Path(__file__).resolve().parent.parent / "kat" / "vectors.json"
path.write_text(json.dumps({"scheme": "CAIRN v1", "cases": out}, indent=1) + "\n")
print(f"wrote {len(out)} cases to {path}")
