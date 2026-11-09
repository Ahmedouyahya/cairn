"""Write video/youtube.md: title, description and chapters from build/durations.json."""

import json
from pathlib import Path

VID = Path(__file__).resolve().parent
d = json.loads((VID / "build" / "durations.json").read_text())
names = {
    "01_hook": "Why secret sharing is not enough",
    "02_windows": "Five windows through which a shared key leaks",
    "03_field": "The Mersenne field GF(2^127 - 1)",
    "04_amd": "L2: algebraic manipulation detection",
    "05_sharing": "L3: Vandermonde sharing",
    "06_storage": "L4: split-state inner-product storage",
    "07_refresh": "L5: skew-symmetric refresh, and the break we found",
    "08_sidechannel": "L6: constant time, measured",
    "09_demo": "Demo: the cairn tool against real attacks",
    "10_close": "Evidence and reproducibility",
}
t, chapters = 0.0, []
for key, v in d.items():
    chapters.append(f"{int(t) // 60}:{int(t) % 60:02d} {names[key]}")
    t += v["total"]
nl = "\n"
(VID / "youtube.md").write_text(f"""# YouTube upload

**Title:** CAIRN: information-theoretic, leakage-resilient threshold custody of master keys

**Visibility:** Public or Unlisted (either works for Devpost).

**Description** (paste as is):

CAIRN keeps a master key under t-of-n custody using only linear algebra over the Mersenne field GF(2^127 - 1), with no block cipher, no hash function and no hardness assumption. This video walks through the six layers, the theorems behind them, the cross-refresh attack we found in our own first design, and the measurements that back every claim.

Entry for the Mathematics for Systems Security hackathon (Cryptographer track).

Code, paper and all results: https://github.com/Ahmedouyahya/cairn
Paper (PDF): https://github.com/Ahmedouyahya/cairn/blob/main/paper/cairn.pdf

Chapters
{nl.join(chapters)}

Every number on screen is read from the lab's results files, and every terminal line is real output of the cairn binary. The animation was rendered with Manim Community.
Voice-over generated with ElevenLabs (elevenlabs.io).

**Subtitles:** upload `video/build/cairn.srt` (Subtitles, English, "Upload file", "With timing").

**Length:** {int(t) // 60}:{int(t) % 60:02d}
""")
print(nl.join(chapters), f"\ntotal {int(t) // 60}:{int(t) % 60:02d}")
