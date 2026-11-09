"""Original sound design for the CAIRN video, synthesized from scratch.

* music(): a slow ambient pad (Am9, Fmaj7, Cmaj7, Em7), quiet, with a soft
  high shimmer. Nothing sampled, so there is no licensing question.
* sfx: a key tick, a soft pop, a gentle chime and a low tone, used at the
  cue points each scene records (build/cues/<key>.json).
* mix(): voice + ducked music + effects, ready for loudness normalisation.
"""

import numpy as np

SR = 44100


def _env(n, attack, release):
    e = np.ones(n)
    a, r = int(attack * SR), int(release * SR)
    if a:
        e[:a] = np.linspace(0, 1, a)
    if r:
        e[-r:] *= np.linspace(1, 0, r)
    return e


def music(seconds, seed=7):
    rng = np.random.default_rng(seed)
    midi = lambda m: 440.0 * 2 ** ((m - 69) / 12)  # noqa: E731
    chords = [[45, 57, 60, 64, 71], [41, 53, 57, 64, 69], [48, 55, 59, 64, 71], [40, 52, 55, 59, 67]]
    bar = 9.0  # seconds per chord
    n = int(seconds * SR)
    t = np.arange(n) / SR
    out = np.zeros((n, 2))
    k = 0
    start = 0.0
    while start < seconds:
        notes = chords[k % len(chords)]
        s, e = int(start * SR), min(n, int((start + bar + 3.0) * SR))  # 3 s overlap
        tt = t[s:e] - start
        env = _env(e - s, 2.5, 3.5)
        for j, m in enumerate(notes):
            f = midi(m)
            for det, pan in ((-0.0025, 0.3), (0.0025, 0.7)):  # gentle chorus
                ph = rng.uniform(0, 2 * np.pi)
                v = np.sin(2 * np.pi * f * (1 + det) * tt + ph) * (0.5 if j == 0 else 0.22)
                v += 0.06 * np.sin(2 * np.pi * 2 * f * tt + ph)  # a little brightness
                out[s:e, 0] += v * env * (1 - pan)
                out[s:e, 1] += v * env * pan
        start += bar
        k += 1
    # soft shimmer: slow high sine bells, sparse
    for _ in range(int(seconds / 6)):
        at = rng.uniform(0, seconds - 3)
        f = midi(rng.choice([76, 79, 81, 83, 84, 88]))
        s, e = int(at * SR), int((at + 3) * SR)
        tt = t[s:e] - at
        bell = np.sin(2 * np.pi * f * tt) * np.exp(-tt * 1.6) * 0.05
        pan = rng.uniform(0.2, 0.8)
        out[s:e, 0] += bell * (1 - pan)
        out[s:e, 1] += bell * pan
    out /= np.max(np.abs(out)) + 1e-9
    return out * 0.5


def _tone(f, dur, decay, amp, f_end=None):
    t = np.arange(int(dur * SR)) / SR
    freq = f if f_end is None else np.linspace(f, f_end, len(t))
    phase = 2 * np.pi * np.cumsum(freq) / SR
    return np.sin(phase) * np.exp(-t * decay) * amp * _env(len(t), 0.004, 0.02)


def sfx(name, dur=0.0, seed=0):
    rng = np.random.default_rng(seed)
    if name == "tick":  # one soft key press
        n = int(0.018 * SR)
        x = rng.normal(0, 1, n) * np.exp(-np.arange(n) / SR * 260)
        return np.diff(np.concatenate([[0], x])) * 0.06
    if name == "type":  # a burst of key presses spread over `dur`
        total = np.zeros(int((dur + 0.05) * SR))
        t = 0.0
        while t < dur:
            k = sfx("tick", seed=int(t * 1000) + seed)
            i = int(t * SR)
            total[i:i + len(k)] += k[: len(total) - i] * rng.uniform(0.6, 1.0)
            t += rng.uniform(0.045, 0.09)
        return total
    if name == "pop":
        return _tone(740, 0.16, 28, 0.10, f_end=520)
    if name == "chime":
        return _tone(1318.5, 1.6, 2.6, 0.045) + _tone(1975.5, 1.6, 3.2, 0.025)
    if name == "low":
        return _tone(196, 0.35, 9, 0.12, f_end=165)
    raise ValueError(name)


def envelope(x, win=0.25):
    n = int(win * SR)
    k = np.ones(n) / n
    return np.convolve(np.abs(x), k, mode="same")


def mix(voice, cues, music_gain_db=-21.0, duck_db=-9.0):
    """voice: mono float array (whole video). cues: list of (time, name, dur)."""
    n = len(voice)
    bed = music(n / SR + 1)[:n]
    # duck the music when the voice is present (smoothed envelope)
    env = envelope(voice, 0.35)
    active = np.clip(env / (np.percentile(env, 95) * 0.35 + 1e-9), 0, 1)
    active = np.convolve(active, np.ones(int(0.4 * SR)) / int(0.4 * SR), mode="same")
    gain = 10 ** ((music_gain_db + duck_db * active) / 20)
    fade = _env(n, 2.5, 4.0)
    out = np.zeros((n, 2))
    out += bed * (gain * fade)[:, None]
    out += voice[:, None]
    for t, name, dur in cues:
        s = sfx(name, dur, seed=int(t * 100))
        i = int(t * SR)
        if i >= n:
            continue
        m = min(len(s), n - i)
        out[i:i + m, 0] += s[:m]
        out[i:i + m, 1] += s[:m]
    peak = np.max(np.abs(out))
    if peak > 0.98:
        out *= 0.98 / peak
    return out
