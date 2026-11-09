# YouTube upload

**Live:** https://youtu.be/_m6iQ6-VnBc (public)

**Title:** CAIRN: information-theoretic, leakage-resilient threshold custody of master keys

**Visibility:** Public or Unlisted (either works for Devpost).

**Description** (paste as is):

CAIRN keeps a master key under t-of-n custody using only linear algebra over the Mersenne field GF(2^127 - 1), with no block cipher, no hash function and no hardness assumption. This video walks through the six layers, the theorems behind them, the cross-refresh attack we found in our own first design, and the measurements that back every claim.

Entry for the Mathematics for Systems Security hackathon (Cryptographer track).

Code, paper and all results: https://github.com/Ahmedouyahya/cairn
Paper (PDF): https://github.com/Ahmedouyahya/cairn/blob/main/paper/cairn.pdf

Chapters
0:00 Why secret sharing is not enough
0:25 Five windows through which a shared key leaks
1:06 The Mersenne field GF(2^127 - 1)
1:31 L2: algebraic manipulation detection
2:02 L3: Vandermonde sharing
2:23 L4: split-state inner-product storage
2:53 L5: skew-symmetric refresh, and the break we found
3:41 L6: constant time, measured
4:10 Demo: the cairn tool against real attacks
4:47 Evidence and reproducibility

Every number on screen is read from the lab's results files, and every terminal line is real output of the cairn binary. The animation was rendered with Manim Community.
Voice-over generated with ElevenLabs (elevenlabs.io).

**Subtitles:** upload `video/build/cairn.srt` (Subtitles, English, "Upload file", "With timing").

**Length:** 5:17
