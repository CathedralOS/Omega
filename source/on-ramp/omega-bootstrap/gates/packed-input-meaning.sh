#!/usr/bin/env sh
# Packed Delta-input transport: actual Gamma equivalence and resource boundaries.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
ENCODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/encode-gamma-input.py"
stamp_beta_compiler "$T/bc.exe" >/dev/null
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null
  "$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED" < "$T/program.asm" > "$T/program.tape" 2>/dev/null
  stamp_seed "$T/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe"
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe"

"$T/elaborate.exe" < "$OMEGA_PATH_CORPUS/stdin_checksum/main.omg" > "$T/template.gamma"
[ "$(grep -o STDIN "$T/template.gamma" | wc -l | tr -d ' ')" -eq 1 ]

python3 - "$T" <<'PY'
from pathlib import Path
import sys

out = Path(sys.argv[1])
cases = [b"", b"\0", b"\0\xff", b"abc", b"abcd", b"abcde", bytes(range(256))]
for index, payload in enumerate(cases):
    (out / f"case-{index}.input").write_bytes(payload)
(out / "maximum.input").write_bytes(bytes(index & 0xFF for index in range(524_288)))
(out / "plus-one.input").write_bytes(bytes(index & 0xFF for index in range(524_289)))
PY

PASS=0
for INPUT in "$T"/case-*.input; do
  python3 "$ENCODER" inject "$T/template.gamma" "$INPUT" "$T/packed.gamma"
  python3 "$ENCODER" inject-cons "$T/template.gamma" "$INPUT" "$T/cons.gamma"
  set +e
  "$T/interp.exe" < "$T/packed.gamma" > "$T/packed.out"
  PACKED_STATUS=$?
  "$T/interp.exe" < "$T/cons.gamma" > "$T/cons.out"
  CONS_STATUS=$?
  set -e
  [ "$PACKED_STATUS" -eq "$CONS_STATUS" ] && cmp "$T/packed.out" "$T/cons.out" || {
    echo "packed input meaning: mismatch for $(wc -c < "$INPUT" | tr -d ' ') bytes" >&2
    exit 1
  }
  PASS=$((PASS + 1))
done

# Exercise the maximal generated carrier through the canonical interpreter,
# independently of a Delta program's much larger execution/resource profile.
printf '%s\n' '(match STDIN ((Chunks n t) n))' > "$T/extent.template"
python3 "$ENCODER" inject "$T/extent.template" "$T/maximum.input" "$T/maximum.gamma"
set +e
"$T/interp.exe" < "$T/maximum.gamma" > "$T/maximum.out"
MAX_STATUS=$?
set -e
[ "$MAX_STATUS" -eq 0 ] && [ "$(tr -d '\n' < "$T/maximum.out")" = 524288 ] || {
  echo "packed input meaning: maximal carrier was not interpreted exactly" >&2
  exit 1
}

set +e
python3 "$ENCODER" inject "$T/extent.template" "$T/plus-one.input" "$T/plus-one.gamma" \
  > "$T/plus-one.stdout" 2> "$T/plus-one.stderr"
PLUS_ONE_STATUS=$?
set -e
[ "$PLUS_ONE_STATUS" -eq 252 ] && [ ! -s "$T/plus-one.gamma" ] || {
  echo "packed input meaning: 524288+1 did not refuse without a carrier" >&2
  exit 1
}

echo "packed input meaning: $PASS Cons-equivalent interpreter cases, all 256 bytes, exact maximum, and +1 refusal passed"
