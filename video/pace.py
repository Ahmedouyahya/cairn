"""Give the narration room to breathe.

For each voice clip, find the silences between sentences, cut the clip at
the silence nearest to each expected sentence boundary, and rebuild it with a
fixed pause after every sentence. Writes build/paced/<key>.wav and returns
the exact start time of every sentence in the paced audio, so each scene can
put a visual on screen exactly when its sentence begins.
"""

import re
import subprocess
from pathlib import Path

import numpy as np
import soundfile as sf

SR = 44100
FRAME = 0.02  # seconds per RMS frame


def decode(path: Path) -> np.ndarray:
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32).copy()


def silences(y: np.ndarray, min_len=0.12, rel_db=-38.0):
    n = int(FRAME * SR)
    frames = len(y) // n
    rms = np.sqrt(np.mean(y[: frames * n].reshape(frames, n) ** 2, axis=1) + 1e-12)
    quiet = rms < rms.max() * 10 ** (rel_db / 20)
    gaps, start = [], None
    for i, q in enumerate(np.append(quiet, False)):
        if q and start is None:
            start = i
        elif not q and start is not None:
            if (i - start) * FRAME >= min_len:
                gaps.append((start * FRAME, i * FRAME))
            start = None
    return gaps


def sentences(text: str):
    return [s for s in re.split(r"(?<=[.!?])\s+", text.strip()) if s]


def pace(voice: Path, text: str, out: Path, lead: float, pause: float, tail: float):
    y = decode(voice)
    dur = len(y) / SR
    gaps = silences(y)
    # speech span: drop leading/trailing silence
    s0 = gaps[0][1] if gaps and gaps[0][0] < 0.05 else 0.0
    s1 = gaps[-1][0] if gaps and gaps[-1][1] > dur - 0.05 else dur
    inner = [g for g in gaps if g[0] > s0 and g[1] < s1]
    sents = sentences(text)
    words = [len(s.split()) for s in sents]
    cum = np.cumsum(words)[:-1] / sum(words)
    cuts, used = [], set()
    for frac in cum:  # nearest unused silence after the previous cut
        expect = s0 + frac * (s1 - s0)
        cands = [(abs((a + b) / 2 - expect), k) for k, (a, b) in enumerate(inner)
                 if k not in used and (not cuts or (a + b) / 2 > cuts[-1] + 0.3)]
        if not cands:
            cuts.append(expect)
            continue
        _, k = min(cands)
        used.add(k)
        cuts.append((inner[k][0] + inner[k][1]) / 2)
    bounds = [s0] + cuts + [s1]
    pieces, starts, t = [np.zeros(int(lead * SR), np.float32)], [], lead
    for i in range(len(sents)):
        seg = y[int(bounds[i] * SR): int(bounds[i + 1] * SR)]
        starts.append(round(t, 3))
        pieces.append(seg)
        t += len(seg) / SR
        gap = pause if i < len(sents) - 1 else tail
        pieces.append(np.zeros(int(gap * SR), np.float32))
        t += gap
    speech_end = round(t - tail, 3)
    out.parent.mkdir(parents=True, exist_ok=True)
    sf.write(out, np.concatenate(pieces), SR)
    return {"starts": starts + [speech_end], "total": round(t, 3), "cuts": [round(c, 3) for c in cuts],
            "matched": len(used), "sentences": len(sents)}
