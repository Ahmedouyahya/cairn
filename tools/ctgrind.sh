#!/usr/bin/env bash
# ctgrind: secret-dependent branches and memory accesses, found by Valgrind.
# CAIRN must produce zero memcheck errors; the naive baseline must produce some.
set -euo pipefail
cd "$(dirname "$0")/.."
LAB=target/release/cairn-lab
if ! command -v valgrind >/dev/null; then
  echo "ctgrind: valgrind not installed (sudo dnf install valgrind); skipping"
  exit 0
fi
mkdir -p results
set +e
valgrind --error-exitcode=42 --track-origins=yes "$LAB" ctgrind-cairn > results/ctgrind-cairn.txt 2>&1
cairn=$?
valgrind --error-exitcode=42 --track-origins=yes "$LAB" ctgrind-naive > results/ctgrind-naive.txt 2>&1
naive=$?
set -e
# Keep logs free of local paths.
sed -i "s|$PWD/||g" results/ctgrind-cairn.txt results/ctgrind-naive.txt
grep "ERROR SUMMARY" results/ctgrind-cairn.txt | sed 's/^/  cairn: /'
grep "ERROR SUMMARY" results/ctgrind-naive.txt | sed 's/^/  naive: /'
if [ "$cairn" -eq 0 ] && [ "$naive" -eq 42 ]; then
  echo "[PASS] ctgrind  ->  results/ctgrind-*.txt"
else
  echo "[FAIL] ctgrind (cairn exit $cairn, naive exit $naive)"; exit 1
fi
