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
import sys
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VID = ROOT / "video"
OUT = VID / "build"
NARR = VID / "narration"
VOICE = VID / "voice"
LEAD, PAUSE, TAIL, WPS = 0.8, 0.9, 2.2, 2.45
# Longer holds where the viewer needs time to read a result.
TAILS = {"01_hook": 3.2, "07_refresh": 3.0, "08_sidechannel": 2.8, "09_demo": 3.0, "10_close": 4.2}

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
    ap.add_argument("--mux-only", action="store_true", help="reuse existing renders, only redo audio and joining")
    a = ap.parse_args()
    OUT.mkdir(exist_ok=True)

    sys.path.insert(0, str(VID))
    from pace import pace

    durs = {}
    for key, _ in SCENES:
        vf = voice_file(key)
        tail = TAILS.get(key, TAIL)
        text = (NARR / f"{key}.txt").read_text()
        if vf:
            d = pace(vf, text, OUT / "paced" / f"{key}.wav", LEAD, PAUSE, tail)
            print(f"  {key}: {d['matched']}/{d['sentences'] - 1} sentence breaks found in the audio")
        else:  # no voice yet: estimate from word counts
            sents = sentences(key)
            starts, t = [], LEAD
            for s in sents:
                starts.append(round(t, 3))
                t += len(s.split()) / WPS + PAUSE
            d = {"starts": starts + [round(t - PAUSE, 3)], "total": round(t - PAUSE + tail, 3)}
        durs[key] = {**d, "lead": LEAD, "has_voice": bool(vf)}
    (OUT / "durations.json").write_text(json.dumps(durs, indent=1) + "\n")
    total = sum(d["total"] for d in durs.values())
    print(f"total length {total/60:.0f}:{total%60:04.1f}  (voice clips: {sum(d['has_voice'] for d in durs.values())}/10)")

    res, fps, tag = ("854,480", 30, "480p") if a.draft else ("1920,1080", 30, "1080p")
    manim = ["uv", "run", "--python", "3.12", "--with", "manim==0.19.0", "manim"]
    parts = []
    for key, cls in SCENES:
        if a.only and a.only != cls:
            continue
        if not a.mux_only:
            sh(manim + ["render", "--resolution", res, "--frame_rate", str(fps), "--media_dir", str(OUT / "media"),
                        "--disable_caching", "-o", f"{key}.mp4", str(VID / "scenes.py"), cls], stdout=subprocess.DEVNULL)
        # Manim writes to a folder named after the resolution, e.g. 1080p30.
        video = OUT / "media" / "videos" / "scenes" / f"{res.split(',')[1]}p{fps}" / f"{key}.mp4"
        d = durs[key]
        out = OUT / f"part_{key}.mp4"
        # Video only, cut or padded (last frame held) to the scene's exact length.
        sh(["ffmpeg", "-y", "-v", "error", "-i", str(video), "-vf", "tpad=stop_mode=clone:stop_duration=3",
            "-t", str(d["total"]), "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "14",
            "-pix_fmt", "yuv420p", "-r", str(fps), str(out)])
        parts.append(out)
        print(f"  rendered {key}  ({d['total']:.1f} s)")

    if a.only:
        return
    lst = OUT / "parts.txt"
    lst.write_text("".join(f"file '{p.name}'\n" for p in parts))
    joined = OUT / "joined_video.mp4"
    sh(["ffmpeg", "-y", "-v", "error", "-f", "concat", "-safe", "0", "-i", str(lst), "-c", "copy", str(joined)], cwd=OUT)

    # One audio mix for the whole film: paced voice + ducked music + effects at the scenes' cues.
    import numpy as np
    import soundfile as sf
    from sound import SR, mix

    voice, cues, offset = [], [], 0.0
    for key, _ in SCENES:
        d = durs[key]
        n = int(round(d["total"] * SR))
        if d["has_voice"]:
            y, sr = sf.read(OUT / "paced" / f"{key}.wav", dtype="float32")
            assert sr == SR
        else:
            y = np.zeros(n, np.float32)
        y = np.pad(y, (0, max(0, n - len(y))))[:n]
        voice.append(y)
        cf = OUT / "cues" / f"{key}.json"
        if cf.exists():
            cues += [(offset + t, name, dur) for t, name, dur in json.loads(cf.read_text())]
        offset += n / SR
    voice = np.concatenate(voice)
    voice *= 0.5 / (np.max(np.abs(voice)) + 1e-9)  # headroom; loudness is normalised at the end
    sf.write(OUT / "mix.wav", mix(voice, cues), SR)
    print(f"  mixed audio: {len(cues)} sound cues")

    final = OUT / f"cairn_{tag}{'' if all(d['has_voice'] for d in durs.values()) else '_silent_draft'}.mp4"
    sh(["ffmpeg", "-y", "-v", "error", "-i", str(joined), "-i", str(OUT / "mix.wav"),
        "-vf", "vignette=angle=PI/9:mode=forward,noise=alls=3:allf=t",
        "-af", "loudnorm=I=-16:TP=-1.5:LRA=11,aresample=44100",
        "-map", "0:v", "-map", "1:a", "-shortest",
        "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart", str(final)])

    # Subtitles, sentence by sentence, using the same timing model as the scenes.
    srt, n, offset = [], 1, 0.0
    for key, _ in SCENES:
        d = durs[key]
        sents = sentences(key)
        for i, s in enumerate(sents):
            start = offset + d["starts"][i]
            end = offset + (d["starts"][i + 1] - (PAUSE if i < len(sents) - 1 else 0))
            srt.append(f"{n}\n{stamp(start)} --> {stamp(end)}\n{s}\n")
            n += 1
        offset += d["total"]
    (OUT / "cairn.srt").write_text("\n".join(srt))
    print(f"video: {final}\nsubtitles: {OUT / 'cairn.srt'}")


if __name__ == "__main__":
    main()
