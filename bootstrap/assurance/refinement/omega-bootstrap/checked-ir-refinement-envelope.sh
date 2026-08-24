#!/usr/bin/env sh
# Focused lower-rooted custody contract for the raw §10.6 refinement envelope.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

for TOOL in python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement envelope: skipped ($TOOL absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null

cp "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" "$T/check.beta"
printf '\nproc main() { return refinement_envelope_read() }\n' >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

printf 'bundle' > "$T/bundle"
printf 'ckir' > "$T/ckir"
printf 'elf' > "$T/elf"
: > "$T/empty"

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir_refinement_bundle.py" \
  "$T/bundle" "$T/ckir" "$T/elf" --result 70 > "$T/entry.rfn"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir_refinement_bundle.py" \
  "$T/bundle" "$T/ckir" "$T/empty" --library > "$T/library.rfn"

observe() {
  expected=$1
  input=$2
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement envelope: $input returned $actual, expected $expected" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement envelope: $input published stdout" >&2
    exit 1
  }
}

observe 0 "$T/entry.rfn"
observe 0 "$T/library.rfn"

python3 - "$T/entry.rfn" "$T/bad-magic" "$T/bad-exit" "$T/truncated" "$T/trailing" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
bad_magic = bytearray(canonical)
bad_magic[0] ^= 1
Path(sys.argv[2]).write_bytes(bad_magic)
bad_exit = bytearray(canonical)
struct.pack_into("<I", bad_exit, 32, 71)
Path(sys.argv[3]).write_bytes(bad_exit)
Path(sys.argv[4]).write_bytes(canonical[:-1])
Path(sys.argv[5]).write_bytes(canonical + b"\0")
PY

observe 251 "$T/bad-magic"
observe 251 "$T/bad-exit"
observe 251 "$T/truncated"
observe 251 "$T/trailing"

python3 - "$T/oversized" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_bytes(b"\0" * (4 * 1024 * 1024 + 1))
PY
observe 252 "$T/oversized"

echo "checked-IR refinement envelope: exact entry/library framing, claims, EOF, and exhaustion passed below Delta"
