#!/usr/bin/env bash
# Paced terminal demo for the video. Run from the repository root after
# `cargo build --release`. Press Enter to advance between scenes.
# Usage: video/demo.sh            (interactive)
#        PAUSE=0 video/demo.sh    (no pauses, for a dry run)
set -euo pipefail
cd "$(dirname "$0")/.."
CAIRN="$PWD/target/release/cairn"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
OLDPWD_REPO="$PWD"; cd "$WORK"; OLDPWD="$OLDPWD_REPO"

bold=$(tput bold 2>/dev/null || true); dim=$(tput dim 2>/dev/null || true); reset=$(tput sgr0 2>/dev/null || true)
scene() { echo; echo "${bold}== $*${reset}"; [ "${PAUSE:-1}" = 0 ] || read -r -p "${dim}(enter)${reset}" _; }
run() { echo "${dim}\$ $*${reset}"; "$@"; }
cairn() { "$CAIRN" "$@"; }

scene "1. A fresh 32-byte master key, split 3-of-5. It is never printed and never written to disk."
run "$CAIRN" split -t 3 -n 5 --generate 32 -o shares
run ls -l shares
run "$CAIRN" inspect shares/share-1.cairn

scene "2. Use the key without ever seeing it: any 3 custodians, piped straight into a program."
run "$CAIRN" combine shares/share-1.cairn shares/share-3.cairn shares/share-5.cairn --exec sha256sum
run "$CAIRN" combine shares/share-2.cairn shares/share-3.cairn shares/share-4.cairn --exec sha256sum

scene "3. Custodian 1 cheats (Tompa-Woll): she alters one byte of her share."
cp shares/share-1.cairn honest-1.cairn
run sh -c "printf '\\x01' | dd of=shares/share-1.cairn bs=1 seek=700 conv=notrunc status=none"
run "$CAIRN" combine shares/share-1.cairn shares/share-2.cairn shares/share-3.cairn --exec sha256sum || true
scene "   ...and with a fourth share present, the key is recovered and the cheater is named."
run "$CAIRN" combine shares/share-1.cairn shares/share-2.cairn shares/share-3.cairn shares/share-4.cairn --exec sha256sum
cp honest-1.cairn shares/share-1.cairn

scene "4. The mobile adversary: steal share 1 now, refresh, steal shares 2 and 3 later."
run cp shares/share-1.cairn stolen-1.cairn
run "$CAIRN" refresh shares/share-1.cairn shares/share-2.cairn shares/share-3.cairn shares/share-4.cairn shares/share-5.cairn
run "$CAIRN" combine stolen-1.cairn shares/share-2.cairn shares/share-3.cairn --exec sha256sum || true
run "$CAIRN" combine shares/share-1.cairn shares/share-2.cairn shares/share-3.cairn --exec sha256sum

scene "5. Same attacks against textbook Shamir, and against CAIRN, from the attack suite."
run python3 -I "$OLDPWD/attacks/tompa_woll.py"

scene "6. Timing side channel: fixed vs random inputs (Welch t-test). Threshold 4.5."
run "$OLDPWD/target/release/cairn-lab" dudect

scene "7. The exhaustive proofs and the refresh break we found in our own first design."
run "$OLDPWD/target/release/cairn-lab" refresh

echo; echo "${bold}Everything above, plus the paper: make verify${reset}"
