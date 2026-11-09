# Voice-over with ElevenLabs

The video has ten scenes. Each scene has its narration in `video/narration/NN_name.txt`. Generate one audio file per scene and save it here with the **same base name**, for example `01_hook.mp3`, `02_windows.mp3`, ... `10_close.mp3`. The render script measures each file and times its scene to match, so pauses and pacing are yours to choose.

## Voice

Use **Voice Design** (Voices → Add voice → Voice Design) with this description:

> A young African man in his mid-twenties, a mathematics and security researcher. Warm, clear and confident, with a light West African English accent. Calm, measured pace, like a researcher presenting his own work at a conference. Natural and articulate, not dramatic, not salesy. Studio-quality recording, close microphone, no background noise.

Generate a few previews and pick the one that sounds most natural on the math sentences (scene 03 is a good test).

## Settings

- **Model:** Eleven Multilingual v2 (most natural for English narration). Eleven v3 is also fine if you prefer it.
- **Stability:** about 50%. Lower sounds more expressive; higher sounds more even.
- **Similarity:** about 75%.
- **Style exaggeration:** 0 to 15%.
- **Speed:** 0.95 to 1.0.
- **Output:** MP3, 44.1 kHz, 128 kbps or higher.

## Pronunciation notes

- **CAIRN** is said like *"kern"* (rhymes with *"learn"*). If the voice says it wrong, type it as `Kern` for that generation only.
- *"ell"* is the letter L, used for the vector length.
- *"Lindsey's lemma"*, *"Vandermonde"* (van-der-MOND) and *"Valgrind"* (VAL-grinned) sometimes need one or two regenerations.
- Numbers are already written out in words in the text files, so the voice reads them correctly.

## When the files are here

```console
$ make video        # renders every scene timed to the voice and muxes the audio
```

Without audio files, `make video` renders a silent draft timed by word count.
