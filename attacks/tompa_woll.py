"""Attack 1: Tompa-Woll cheating (J. Cryptology 1988).

Setting: 3-of-5 custody of a 32-byte master key. Custodians 1, 2 and 3 meet to
unlock. Custodian 1 is malicious: instead of their share they submit
`share + Delta`.

* Plain Shamir: reconstruction is linear, so the group recovers the WRONG key
  `s' = s + lambda_1 * Delta`. The cheater, who knows `lambda_1` and `Delta`,
  computes the real key `s = s' - lambda_1 * Delta` and is now the only person
  who has it.
* CAIRN (the real `cairn` binary): the AMD layer turns the same additive error
  into a rejection with probability at least 1 - (k+1)/p, about 1 - 2^-125.
  Nothing is output, so the cheater learns nothing. With a fourth share
  present, CAIRN recovers the key and names the cheater.

Run: python3 -I attacks/tompa_woll.py   (after `cargo build --release`)
"""

import os
import secrets
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "reference"))
from cairn_ref import P, Share, inv, lagrange_at_zero  # noqa: E402

CAIRN = ROOT / "target" / "release" / "cairn"


def banner(s):
    print(f"\n=== {s}")


def plain_shamir_attack() -> bool:
    banner("plain Shamir (textbook): 3-of-5, cheater is custodian 1")
    key = int.from_bytes(secrets.token_bytes(15), "big")  # one limb, for clarity
    coeffs = [key, secrets.randbelow(P), secrets.randbelow(P)]
    share = {x: sum(c * pow(x, i, P) for i, c in enumerate(coeffs)) % P for x in range(1, 6)}

    delta = secrets.randbelow(P)
    submitted = {1: (share[1] + delta) % P, 2: share[2], 3: share[3]}
    lam = dict(zip([1, 2, 3], lagrange_at_zero([1, 2, 3])))
    published = sum(lam[x] * submitted[x] for x in submitted) % P

    stolen = (published - lam[1] * delta) % P
    print(f"  honest custodians recovered : {published:#034x}  (wrong)")
    print(f"  cheater computes            : {stolen:#034x}")
    print(f"  real key                    : {key:#034x}")
    ok = stolen == key and published != key
    print("  RESULT: cheater stole the key, honest parties were fooled" if ok else "  unexpected")
    return ok


def tamper(path: Path, delta: int) -> None:
    """Add `delta` to one element of the R half (keeps the file well-formed)."""
    s = Share.from_bytes(path.read_bytes())
    s.right[0][0] = (s.right[0][0] + delta) % P
    path.write_bytes(s.to_bytes())


def run(args, **kw):
    return subprocess.run([str(CAIRN), *args], capture_output=True, **kw)


def cairn_attack() -> bool:
    banner("CAIRN (real binary): same attack")
    if not CAIRN.exists():
        sys.exit("build first: cargo build --release -p cairn-cli")
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        key = secrets.token_bytes(32)
        (d / "key.bin").write_bytes(key)
        assert run(["split", "-t", "3", "-n", "5", "--in", str(d / "key.bin"), "-o", str(d / "s")]).returncode == 0
        os.remove(d / "key.bin")
        sh = lambda i: str(d / "s" / f"share-{i}.cairn")  # noqa: E731

        tamper(Path(sh(1)), secrets.randbelow(P - 1) + 1)
        r = run(["combine", sh(1), sh(2), sh(3), "--out", "-"])
        print(f"  combine 1,2,3 -> exit {r.returncode}, {len(r.stdout)} bytes of output")
        print(f"  stderr: {r.stderr.decode().strip()}")
        detected = r.returncode != 0 and r.stdout == b""

        r = run(["combine", sh(1), sh(2), sh(3), sh(4), "--out", "-"])
        print(f"  combine 1,2,3,4 -> exit {r.returncode}")
        for line in r.stderr.decode().strip().splitlines():
            print(f"  stderr: {line}")
        recovered = r.returncode == 0 and r.stdout == key and b"inconsistent share(s): 1 " in r.stderr

        # Many independent trials: the AMD bound says no tampering should ever pass.
        trials, passed = 200, 0
        for _ in range(trials):
            s = Share.from_bytes(Path(sh(2)).read_bytes())
            s.left[secrets.randbelow(s.dim)][secrets.randbelow(s.ell)] = secrets.randbelow(P)
            Path(d / "t.cairn").write_bytes(s.to_bytes())
            r = run(["combine", sh(1).replace("share-1", "share-5"), str(d / "t.cairn"), sh(3), "--out", "-"])
            passed += r.returncode == 0 and r.stdout != key
        print(f"  {trials} random tamperings of share 2: wrong key accepted {passed} times")

    ok = detected and recovered and passed == 0
    print("  RESULT: cheating detected, nothing leaked, cheater identified" if ok else "  RESULT: FAILED")
    return ok


if __name__ == "__main__":
    a = plain_shamir_attack()
    b = cairn_attack()
    print("\nPASS" if a and b else "\nFAIL")
    sys.exit(0 if a and b else 1)
