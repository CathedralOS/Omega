#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Beta addressed regression: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "Beta addressed regression: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
PUBLICATION_TAPE=
PUBLICATION_EXE=
cleanup() {
  rm -rf -- "$T"
  [ -z "$PUBLICATION_TAPE" ] || rm -f -- "$PUBLICATION_TAPE"
  [ -z "$PUBLICATION_EXE" ] || rm -f -- "$PUBLICATION_EXE"
}
trap cleanup EXIT HUP INT TERM
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
materialize_beta_compiler "$T/compiler" >/dev/null

expect_equal() {
  NAME=$1 SOURCE=$2
  "$T/compiler" < "$SOURCE" > "$T/$NAME.tape"
  python3 "$GATE_DIR/beta_ref.py" < "$SOURCE" > "$T/$NAME.ref"
  cmp "$T/$NAME.tape" "$T/$NAME.ref"
}

expect_exit() {
  NAME=$1 EXPECTED=$2
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.exe" >/dev/null
  set +e
  "$T/$NAME.exe" > "$T/$NAME.stdout" 2> "$T/$NAME.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "Beta addressed regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
}

expect_reject() {
  NAME=$1 EXPECTED=$2 SOURCE=$3
  set +e
  "$T/compiler" < "$SOURCE" > "$T/$NAME.rejected" 2> "$T/$NAME.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "Beta addressed regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
}

# Forward and backward numeric control flow with exact output-offset assertions.
printf '%s\n' \
  '0x0:' \
  '        jmp 0xb' \
  '        halt r0' \
  '0xb:' \
  '        imm r0, 0x2' \
  '        imm r1, 0x1' \
  '0x1f:' \
  '        sub r0, r1' \
  '        jnz r0, 0x1f' \
  '0x2c:' \
  '        halt r0' > "$T/numeric-control.beta"
expect_equal numeric-control "$T/numeric-control.beta"
expect_exit numeric-control 0

# Assertions emit nothing and must equal the current output cursor exactly.
printf '%s\n' '0x1:' 'halt r0' > "$T/address-mismatch.beta"
expect_reject address-mismatch 9 "$T/address-mismatch.beta"
printf '%s\n' 'foo:' 'halt r0' > "$T/symbolic-assertion.beta"
expect_reject symbolic-assertion 7 "$T/symbolic-assertion.beta"
printf '%s\n' 'jmp foo' > "$T/symbolic-target.beta"
expect_reject symbolic-target 7 "$T/symbolic-target.beta"
printf '%s\n' '0xA:' 'halt r0' > "$T/uppercase-address.beta"
expect_reject uppercase-address 7 "$T/uppercase-address.beta"
printf '%s\n' '0x:' 'halt r0' > "$T/bare-address.beta"
expect_reject bare-address 7 "$T/bare-address.beta"

# One-pass compilation may expose a nonauthoritative prefix before a late
# assertion failure. Build plumbing must never replace an artifact with it.
printf '%s\n' 'halt r0' '0x3:' > "$T/late-address.beta"
expect_reject late-address 9 "$T/late-address.beta"
[ "$(wc -c < "$T/late-address.rejected" | tr -d ' ')" -eq 2 ]
PUBLICATION_NAME="beta-late-publication-$(basename "$T")"
PUBLICATION_TAPE="$OMEGA_REPO_ROOT/build/tools/beta/$PUBLICATION_NAME.tape"
PUBLICATION_EXE="$OMEGA_REPO_ROOT/build/tools/beta/$PUBLICATION_NAME.exe"
[ ! -e "$PUBLICATION_TAPE" ]
[ ! -e "$PUBLICATION_EXE" ]
cp "$T/late-address.beta" "$T/$PUBLICATION_NAME.beta"
printf 'keep-tape' > "$PUBLICATION_TAPE"
printf 'keep-exe' > "$PUBLICATION_EXE"
set +e
"$OMEGA_REPO_ROOT/tools/bootstrap/beta/build.sh" "$T/$PUBLICATION_NAME.beta" \
  > "$T/publication.stdout" 2> "$T/publication.stderr"
ACTUAL=$?
set -e
[ "$ACTUAL" -eq 9 ]
[ "$(cat "$PUBLICATION_TAPE")" = keep-tape ]
[ "$(cat "$PUBLICATION_EXE")" = keep-exe ]
rm -f -- "$PUBLICATION_TAPE" "$PUBLICATION_EXE"
PUBLICATION_TAPE=
PUBLICATION_EXE=

# Hexadecimal registers and words cover the complete Alpha operand domains.
printf '%s\n' \
  'imm r0, 0xffffffffffffffff' \
  'imm r1, 0x100000' \
  'store r1, r0' \
  'imm r2, 0x7' \
  'add r1, r2' \
  'loadb r0, r1' \
  'halt r0' > "$T/full-word.beta"
expect_equal full-word "$T/full-word.beta"
expect_exit full-word 255

printf '%s\n' 'imm r100, 0x0' > "$T/register-over.beta"
expect_reject register-over 7 "$T/register-over.beta"
printf '%s\n' 'imm rA, 0x0' > "$T/uppercase-register.beta"
expect_reject uppercase-register 7 "$T/uppercase-register.beta"
printf '%s\n' 'imm r0, 1' > "$T/decimal-word.beta"
expect_reject decimal-word 7 "$T/decimal-word.beta"
printf '%s\n' 'imm r0, 0xA' > "$T/uppercase-word.beta"
expect_reject uppercase-word 7 "$T/uppercase-word.beta"
printf '%s\n' 'imm r0, 0x10000000000000000' > "$T/word-too-wide.beta"
expect_reject word-too-wide 7 "$T/word-too-wide.beta"
printf '%s\n' 'halt' > "$T/missing-operand.beta"
expect_reject missing-operand 7 "$T/missing-operand.beta"
printf '%s\n' 'toolongxx r0' > "$T/unknown-mnemonic.beta"
expect_reject unknown-mnemonic 8 "$T/unknown-mnemonic.beta"

# Data bytes participate in the same output cursor and retain the narrow escapes.
printf '%s\n' 'db "A B\0\\\""' '0x6:' 'halt r0' > "$T/data-address.beta"
expect_equal data-address "$T/data-address.beta"
printf '%s\n' 'db, "x"' > "$T/db-comma.beta"
expect_reject db-comma 9 "$T/db-comma.beta"
printf '%s\n' 'db "x\n"' > "$T/db-newline-escape.beta"
expect_reject db-newline-escape 9 "$T/db-newline-escape.beta"
printf 'db "a\001b"\n' > "$T/db-control.beta"
expect_reject db-control 9 "$T/db-control.beta"

# The source and output regions are half-open and checked before advancement.
python3 -c 'import sys; sys.stdout.write(" " * 0x100000)' > "$T/source-full.beta"
"$T/compiler" < "$T/source-full.beta" > "$T/source-full.tape"
[ ! -s "$T/source-full.tape" ]
python3 -c 'import sys; sys.stdout.write(" " * 0x100001)' > "$T/source-over.beta"
expect_reject source-over 9 "$T/source-over.beta"
python3 -c 'import sys; sys.stdout.write("jmp 0x0\n" * 116509)' > "$T/output-over.beta"
expect_reject output-over 9 "$T/output-over.beta"

# The complete byte envelope is checked while loading, including comments.
printf '; comment\000\nhalt r0\n' > "$T/comment-nul.beta"
expect_reject comment-nul 9 "$T/comment-nul.beta"
printf 'halt\013r0\n' > "$T/vertical-tab.beta"
expect_reject vertical-tab 9 "$T/vertical-tab.beta"
printf '; comment\177\nhalt r0\n' > "$T/comment-del.beta"
expect_reject comment-del 9 "$T/comment-del.beta"

echo "Beta addressed regression: numeric control, syntax, and bounds passed ($(wc -c < "$OMEGA_PATH_BETA_COMPILER_TAPE" | tr -d ' ')-byte compiler)"
