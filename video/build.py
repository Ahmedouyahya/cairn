"""Render the CAIRN video.

1. Durations: for each scene, the voice clip video/voice/<key>.(mp3|wav|m4a)
   if present (measured with ffprobe), else a word-count estimate.
2. Render each Manim scene at 1920x1080, 30 fps, timed to that duration.
3. Lay the voice over its scene after a short lead-in, join all scenes.
4. Write subtitles (video/build/cairn.srt) for YouTube.

Usage: python3 video/build.py [--draft] [--only S03Field]
  --draft  renders at 854x480 for a fast preview
"""

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VID = ROOT / "video"
OUT = VID / "build"
NARR = VID / "narration"
VOICE = VID / "voice"
LEAD, TAIL, WPS = 0.6, 1.0, 2.45

SCENES = [
    ("01_hook", "S01Hook"), ("02_windows", "S02Windows"), ("03_field", "S03Field"),
    ("04_amd", "S04Amd"), ("05_sharing", "S05Sharing"), ("06_storage", "S06Storage"),
    ("07_refresh", "S07Refresh"), ("08_sidechannel", "S08SideChannel"),
    ("09_demo", "S09Demo"), ("10_close", "S10Close"),
]


def sh(cmd, **kw):
    return subprocess.run(cmd, check=True, **kw)


def voice_file(key):
    for ext in ("mp3", "wav", "m4a", "ogg"):
        f = VOICE / f"{key}.{ext}"
        if f.exists():
            return f
    return None


def probe(f):
    r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(f)],
                       capture_output=True, text=True, check=True)
    return float(r.stdout.strip())


def sentences(key):
    text = (NARR / f"{key}.txt").read_text().strip()
    return [s for s in re.split(r"(?<=[.!?])\s+", text) if s]


def stamp(t):
    ms = int(round(t * 1000))
    h, ms = divmod(ms, 3_600_000)
    m, ms = divmod(ms, 60_000)
    s, ms = divmod(ms, 1000)
    return f"{h:02}:{m:02}:{s:02},{ms:03}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--draft", action="store_true")
    ap.add_argument("--only")
    a = ap.parse_args()
    OUT.mkdir(exist_ok=True)

    durs = {}
    for key, _ in SCENES:
        vf = voice_file(key)
        voice = probe(vf) if vf else len(" ".join(sentences(key)).split()) / WPS
        durs[key] = {"voice": round(voice, 3), "lead": LEAD, "total": round(LEAD + voice + TAIL, 3), "has_voice": bool(vf)}
    (OUT / "durations.json").write_text(json.dumps(durs, indent=1) + "\n")
    total = sum(d["total"] for d in durs.values())
    print(f"total length {total/60:.0f}:{total%60:04.1f}  (voice clips: {sum(d['has_voice'] for d in durs.values())}/10)")

    res, fps, tag = ("854,480", 30, "480p") if a.draft else ("1920,1080", 30, "1080p")
    manim = ["uv", "run", "--python", "3.12", "--with", "manim==0.19.0", "manim"]
    parts = []
    for key, cls in SCENES:
        if a.only and a.only != cls:
            continue
        sh(manim + ["render", "--resolution", res, "--frame_rate", str(fps), "--media_dir", str(OUT / "media"),
                    "--disable_caching", "-o", f"{key}.mp4", str(VID / "scenes.py"), cls], stdout=subprocess.DEVNULL)
        video = next((OUT / "media" / "videos" / "scenes").glob(f"*/{key}.mp4"))
        d = durs[key]
        vf = voice_file(key)
        out = OUT / f"part_{key}.mp4"
        if vf:
            ms = int(LEAD * 1000)
            audio = ["-i", str(vf), "-filter_complex", f"[1:a]adelay={ms}|{ms},apad[a]", "-map", "0:v", "-map", "[a]"]
        else:
            audio = ["-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo", "-map", "0:v", "-map", "1:a"]
        sh(["ffmpeg", "-y", "-v", "error", "-i", str(video), *audio, "-t", str(d["total"]),
            "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "192k", "-ar", "44100", "-ac", "2", str(out)])
        parts.append(out)
        print(f"  rendered {key}  ({d['total']:.1f} s)")

    if a.only:
        return
    lst = OUT / "parts.txt"
    lst.write_text("".join(f"file '{p.name}'\n" for p in parts))
    final = OUT / f"cairn_{tag}{'' if all(d['has_voice'] for d in durs.values()) else '_silent_draft'}.mp4"
    sh(["ffmpeg", "-y", "-v", "error", "-f", "concat", "-safe", "0", "-i", str(lst), "-c", "copy",
        "-movflags", "+faststart", str(final)], cwd=OUT)

    # Subtitles, sentence by sentence, using the same timing model as the scenes.
    srt, n, offset = [], 1, 0.0
    for key, _ in SCENES:
        d = durs[key]
        sents = sentences(key)
        words = [len(s.split()) for s in sents]
        tot, acc = sum(words), 0
        for s, w in zip(sents, words):
            start = offset + LEAD + d["voice"] * acc / tot
            acc += w
            end = offset + LEAD + d["voice"] * acc / tot
            srt.append(f"{n}\n{stamp(start)} --> {stamp(end)}\n{s}\n")
            n += 1
        offset += d["total"]
    (OUT / "cairn.srt").write_text("\n".join(srt))
    print(f"video: {final}\nsubtitles: {OUT / 'cairn.srt'}")


if __name__ == "__main__":
    main()
