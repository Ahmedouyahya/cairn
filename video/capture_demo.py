"""Run the demo commands for real and record their exact output.

The video's terminal scene replays this transcript, so every line shown on
screen is genuine output of the `cairn` binary. Run: python3 -I video/capture_demo.py
"""

import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAIRN = str(ROOT / "target" / "release" / "cairn")

STEPS = [
    ("cairn split -t 3 -n 5 --generate 32 -o shares/", [CAIRN, "split", "-t", "3", "-n", "5", "--generate", "32", "-o", "shares/"]),
    ("cairn combine shares/share-{1,3,5}.cairn --exec sha256sum",
     [CAIRN, "combine", "shares/share-1.cairn", "shares/share-3.cairn", "shares/share-5.cairn", "--exec", "sha256sum"]),
    ("cairn combine shares/share-{2,3,4}.cairn --exec sha256sum",
     [CAIRN, "combine", "shares/share-2.cairn", "shares/share-3.cairn", "shares/share-4.cairn", "--exec", "sha256sum"]),
    ("cp shares/share-1.cairn stolen-1.cairn", ["cp", "shares/share-1.cairn", "stolen-1.cairn"], "an attacker quietly copies share 1"),
    ("printf '\\x01' | dd of=shares/share-1.cairn bs=1 seek=700 conv=notrunc", 
     ["sh", "-c", "printf '\\001' | dd of=shares/share-1.cairn bs=1 seek=700 conv=notrunc status=none"],
     "custodian 1 cheats: one byte of their share is changed"),
    ("cairn combine shares/share-{1,2,3}.cairn --exec sha256sum",
     [CAIRN, "combine", "shares/share-1.cairn", "shares/share-2.cairn", "shares/share-3.cairn", "--exec", "sha256sum"]),
    ("cairn combine shares/share-{1,2,3,4}.cairn --exec sha256sum",
     [CAIRN, "combine", "shares/share-1.cairn", "shares/share-2.cairn", "shares/share-3.cairn", "shares/share-4.cairn", "--exec", "sha256sum"]),
    ("cp stolen-1.cairn shares/share-1.cairn", ["cp", "stolen-1.cairn", "shares/share-1.cairn"], "custodian 1 restores the honest file"),
    ("cairn refresh shares/share-*.cairn",
     [CAIRN, "refresh"] + [f"shares/share-{i}.cairn" for i in range(1, 6)]),
    ("cairn combine stolen-1.cairn shares/share-{2,3}.cairn --exec sha256sum",
     [CAIRN, "combine", "stolen-1.cairn", "shares/share-2.cairn", "shares/share-3.cairn", "--exec", "sha256sum"],
     "months later: the stolen share plus two fresh ones"),
]

with tempfile.TemporaryDirectory() as d:
    out = []
    for shown, argv, *note in STEPS:
        r = subprocess.run(argv, cwd=d, capture_output=True, text=True)
        lines = [line for line in (r.stdout + r.stderr).splitlines() if line.strip()]
        out.append({"cmd": shown, "note": note[0] if note else "", "out": lines, "ok": r.returncode == 0})

path = ROOT / "video" / "demo_transcript.json"
path.write_text(json.dumps(out, indent=1) + "\n")
for s in out:
    if s["note"]:
        print("#", s["note"])
    print("$", s["cmd"])
    for line in s["out"]:
        print("  ", line)
