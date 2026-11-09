#!/usr/bin/env bash
# Disassemble CAIRN's hot paths from the release binary and count
# conditional jumps and division instructions, following one level of calls.
# Field arithmetic and selection must contain none; the AMD tag may contain
# only loop control over the public number of limbs. Every conditional jump
# is printed with the instruction that sets its flags, for manual review.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=target/release/cairn-lab
out=results/asm_audit.txt
full=results/asm_audit_full.txt
: > "$out"; : > "$full"
dis=$(mktemp); trap 'rm -f "$dis"' EXIT
objdump -d --no-show-raw-insn -M intel "$BIN" > "$dis"

body_of() { awk -v f="<$1>:" '$2==f{p=1;next} p&&/^$/{exit} p' "$dis"; }

audit() {
  local sym=$1 label=$2 body n jcc div
  body=$(body_of "$sym")
  n=$(grep -c . <<<"$body" || true)
  jcc=$(grep -cE '\sj(a|ae|b|be|c|e|g|ge|l|le|na|nae|nb|nbe|nc|ne|ng|nge|nl|nle|no|np|ns|nz|o|p|pe|po|s|z)\s' <<<"$body" || true)
  div=$(grep -cE '\s(i?div)\s' <<<"$body" || true)
  printf "%-34s instructions=%-4s conditional_jumps=%-2s div=%s\n" "$label" "$n" "$jcc" "$div" | tee -a "$out"
  { echo "--- $label ($sym)"; echo "$body"; } >> "$full"
  if [ "$jcc" -gt 0 ]; then
    grep -B1 -E '\sj(a|ae|b|be|c|e|g|ge|l|le|na|nae|nb|nbe|nc|ne|ng|nge|nl|nle|no|np|ns|nz|o|p|pe|po|s|z)\s' <<<"$body" \
      | sed -E 's/^ +[0-9a-f]+:\s+/      /; s/<.*>//' | tee -a "$out"
  fi
  # follow calls into CAIRN code once
  if [ "${label#  }" = "$label" ]; then
    { grep -oE 'call +[0-9a-f]+ <[^>]+>' <<<"$body" || true; } | sed -E 's/.*<(.*)>/\1/' \
      | { grep cairn_core || true; } | sort -u | while read -r callee; do
        audit "$callee" "  -> $(sed -E 's/.*cairn_core[0-9]+([a-z_]+)[0-9]+([a-z_]+).*/\1::\2/' <<<"$callee") (callee)"
      done
  fi
}

for f in cairn_audit_mul cairn_audit_add_sub cairn_audit_select cairn_audit_amd_verify; do
  audit "$f" "$f"
done
echo "[DONE] asm audit  ->  results/asm_audit.txt"
