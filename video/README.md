# Video

The submission video is rendered from code, so every number on screen comes from `results/*.json` and every terminal line is real output of the `cairn` binary (`demo_transcript.json`).

- `narration/NN_name.txt`: the voice-over, one file per scene.
- `voice/`: the ElevenLabs clips (`NN_name.mp3`, not committed); see `voice/README.md`. `voice/choice.json` records which of the two generated takes was used for each scene; takes were chosen by transcribing both with Whisper and comparing them against the script.
- `scenes.py`: the ten Manim scenes.
- `build.py`: times each scene to its voice clip, renders at 1080p, adds the audio and writes subtitles.
- `capture_demo.py`: reruns the demo commands and records their output.
- `demo.sh`: the same story as a live, paced terminal demo.

```console
$ make video-draft   # fast 480p preview (silent if no voice clips yet)
$ make video         # final 1080p video + subtitles in video/build/
```

Rendering needs `ffmpeg`, `uv` and the Pango/Cairo headers (`sudo dnf install pango-devel cairo-devel`).

The voice-over was generated with ElevenLabs (Voice Design, then Eleven v4 text to speech). Upload text, chapters and subtitles for YouTube: `youtube.md`.
