#!/usr/bin/env sh
# Lower-rooted raw source-bundle and lexer custody for the §10.6 source checker.
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

command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR refinement source input: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null

sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/check.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-input.beta" \
  >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
BUNDLER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir_refinement_bundle.py"
printf 'ckir' > "$T/ckir"
printf 'elf' > "$T/elf"

make_case() {
  label=$1
  source=$2
  source_label=${3:-main.omg}
  python3 "$BUNDLER" pack "$source_label=$source" > "$T/$label.bundle"
  python3 "$PACKER" "$T/$label.bundle" "$T/ckir" "$T/elf" --result 70 > "$T/$label.rfn"
}

observe() {
  expected=$1
  input=$2
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement source input: $input returned $actual, expected $expected" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement source input: $input published stdout" >&2
    exit 1
  }
}

make_case fixture "$FIXTURE"
observe 0 "$T/fixture.rfn"

python3 - "$FIXTURE" "$T" <<'PY'
from pathlib import Path
import sys

fixture = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
out.joinpath("comments.omg").write_bytes(
    b"/* outer /* nested */ done */\n" + fixture + b"\n// final comment"
)
out.joinpath("unterminated.omg").write_bytes(fixture + b"\n/* open")
out.joinpath("non-ascii.omg").write_bytes(fixture + b"\n\xff")
out.joinpath("long-ident.omg").write_bytes(b"data " + b"A" * 65 + b" { value: u8; }\n")
out.joinpath("large-int.omg").write_bytes(b"data X { value: u32 [0..=2147483648]; }\n")
out.joinpath("underscore.omg").write_bytes(b"data _Hidden { _value: u8; }\n")
out.joinpath("source-over.omg").write_bytes(b" " * 131073)
PY

make_case comments "$T/comments.omg"
make_case numeric-label "$T/comments.omg" src2/main_1.omg
make_case unterminated "$T/unterminated.omg"
make_case non-ascii "$T/non-ascii.omg"
make_case long-ident "$T/long-ident.omg"
make_case large-int "$T/large-int.omg"
make_case underscore "$T/underscore.omg"
make_case source-over "$T/source-over.omg"

observe 0 "$T/comments.rfn"
observe 0 "$T/numeric-label.rfn"
observe 251 "$T/unterminated.rfn"
observe 251 "$T/non-ascii.rfn"
observe 252 "$T/long-ident.rfn"
observe 251 "$T/large-int.rfn"
observe 0 "$T/underscore.rfn"
observe 252 "$T/source-over.rfn"

python3 - "$T/fixture.rfn" "$T/bad-count.rfn" "$T/bad-label.rfn" <<'PY'
from pathlib import Path
import struct
import sys

canonical = bytearray(Path(sys.argv[1]).read_bytes())
bundle = 36
bad_count = bytearray(canonical)
struct.pack_into("<I", bad_count, bundle + 12, 2)
Path(sys.argv[2]).write_bytes(bad_count)
bad_label = bytearray(canonical)
bad_label[bundle + 24] = ord("/")
Path(sys.argv[3]).write_bytes(bad_label)
PY
observe 251 "$T/bad-count.rfn"
observe 251 "$T/bad-label.rfn"

echo "checked-IR refinement source input: exact bundle, label, source bytes, nested comments, tokens, and limits passed below Delta"
