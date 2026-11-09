# YouTube upload

**Title:** CAIRN: information-theoretic, leakage-resilient threshold custody of master keys

**Visibility:** Public or Unlisted (either works for Devpost).

**Description** (paste as is):

CAIRN keeps a master key under t-of-n custody using only linear algebra over the Mersenne field GF(2^127 - 1), with no block cipher, no hash function and no hardness assumption. This video walks through the six layers, the theorems behind them, the cross-refresh attack we found in our own first design, and the measurements that back every claim.

Entry for the Mathematics for Systems Security hackathon (Cryptographer track).

Code, paper and all results: https://github.com/Ahmedouyahya/cairn
Paper (PDF): https://github.com/Ahmedouyahya/cairn/blob/main/paper/cairn.pdf

Chapters
0:00 Why secret sharing is not enough
0:21 Five windows through which a shared key leaks
0:52 The Mersenne field GF(2^127 - 1)
1:14 L2: algebraic manipulation detection
1:41 L3: Vandermonde sharing
1:56 L4: split-state inner-product storage
2:22 L5: skew-symmetric refresh, and the break we found
3:01 L6: constant time, measured
3:25 Demo: the cairn tool against real attacks
3:53 Evidence and reproducibility

Every number on screen is read from the lab's results files, and every terminal line is real output of the cairn binary. The animation was rendered with Manim Community.
Voice-over generated with ElevenLabs (elevenlabs.io).

**Subtitles:** upload `video/build/cairn.srt` (Subtitles, English, "Upload file", "With timing").

**Length:** 4:19
