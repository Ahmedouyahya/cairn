"""Attack 2: the mobile adversary (Ostrovsky-Yung 1991; Herzberg et al. 1995).

A patient attacker never holds t shares at the same time, but steals one
share now and more shares later. Against static secret sharing that is
enough: after enough months they hold t shares of the same polynomial.

CAIRN's proactive refresh moves every custodian to a fresh, independent
sharing of the same key. Shares from different epochs do not combine:

1. with honest metadata, `cairn` refuses them up front (epoch mismatch);
2. if the attacker rewrites the epoch header, the AMD layer rejects the
   mixture, because the stale share adds an error the attacker cannot predict.

Run: python3 -I attacks/mobile_adversary.py   (after `cargo build --release`)
"""

import secrets
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "reference"))
from cairn_ref import Share  # noqa: E402

CAIRN = ROOT / "target" / "release" / "cairn"


def run(args):
    return subprocess.run([str(CAIRN), *args], capture_output=True)


def main() -> bool:
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        key = secrets.token_bytes(32)
        (d / "key.bin").write_bytes(key)
        run(["split", "-t", "3", "-n", "5", "--in", str(d / "key.bin"), "-o", str(d / "s")])
        sh = lambda i: d / "s" / f"share-{i}.cairn"  # noqa: E731

        print("January : attacker steals share 1 (epoch 0)")
        (d / "stolen-1.cairn").write_bytes(sh(1).read_bytes())

        r = run(["refresh", *[str(sh(i)) for i in range(1, 6)]])
        print(f"February: custodians refresh -> {r.stderr.decode().strip()}")

        print("March   : attacker steals shares 2 and 3 (epoch 1)")
        r = run(["combine", str(d / "stolen-1.cairn"), str(sh(2)), str(sh(3)), "--out", "-"])
        print(f"  combine(stolen 1 @ epoch 0, 2, 3 @ epoch 1) -> exit {r.returncode}: {r.stderr.decode().strip()}")
        refused = r.returncode != 0 and r.stdout == b""

        s = Share.from_bytes((d / "stolen-1.cairn").read_bytes())
        s.epoch = 1
        (d / "forged-1.cairn").write_bytes(s.to_bytes())
        r = run(["combine", str(d / "forged-1.cairn"), str(sh(2)), str(sh(3)), "--out", "-"])
        print(f"  same, with the epoch header forged to 1 -> exit {r.returncode}: {r.stderr.decode().strip()}")
        caught = r.returncode != 0 and r.stdout == b""

        r = run(["combine", str(sh(1)), str(sh(2)), str(sh(3)), "--out", "-"])
        honest = r.returncode == 0 and r.stdout == key
        print(f"  honest custodians at epoch 1 still recover the key: {honest}")

    ok = refused and caught and honest
    print("\nPASS" if ok else "\nFAIL")
    return ok


if __name__ == "__main__":
    sys.exit(0 if main() else 1)
