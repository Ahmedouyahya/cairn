# Video

The submission video is rendered from code, so every number on screen comes from `results/*.json` and every terminal line is real output of the `cairn` binary (`demo_transcript.json`).

- `narration/NN_name.txt`: the voice-over, one file per scene.
- `voice/`: put the ElevenLabs clips here (`NN_name.mp3`); see `voice/README.md`.
- `scenes.py`: the ten Manim scenes.
- `build.py`: times each scene to its voice clip, renders at 1080p, adds the audio and writes subtitles.
- `capture_demo.py`: reruns the demo commands and records their output.
- `demo.sh`: the same story as a live, paced terminal demo.

```console
$ make video-draft   # fast 480p preview (silent if no voice clips yet)
$ make video         # final 1080p video + subtitles in video/build/
```

Rendering needs `ffmpeg`, `uv` and the Pango/Cairo headers (`sudo dnf install pango-devel cairo-devel`).
