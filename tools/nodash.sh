#!/usr/bin/env bash
# House style: no em dashes anywhere in the repository.
cd "$(dirname "$0")/.."
if git grep -n -I $'—' -- . ':!tools/nodash.sh'; then
  echo "[FAIL] em dash found"; exit 1
fi
echo "[PASS] no em dashes"
