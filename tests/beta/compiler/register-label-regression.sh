#!/usr/bin/env sh
# Focused lexical regression: labels beginning r+digit are not registers.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
cd "$GATE_DIR"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Beta assembler register-label regression: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "Beta assembler register-label regression: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf -- "$T"' EXIT HUP INT TERM
START=$(python3 -c 'import time; print(time.time())')
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
materialize_beta_assembler "$T/bootstrap-assembler" >/dev/null
BOOTSTRAP_ASM="$T/bootstrap-assembler"

# Exercise the checked-in Beta source, not merely the previously stamped
# assembler artifact. The old assembler is sufficient to construct this fresh
# candidate; every case below runs the fresh candidate in the Alpha seed.
"$BOOTSTRAP_ASM" < "$OMEGA_PATH_BETA_COMPILER/assembler.beta" \
  > "$T/strict-assembler.tape"
stamp_seed "$T/strict-assembler.tape" "$SEED" "$T/strict-assembler" >/dev/null
ASM="$T/strict-assembler"

expect_equal() {
  NAME=$1 SOURCE=$2
  "$ASM" < "$SOURCE" > "$T/$NAME.tape"
  python3 "$GATE_DIR/asm_ref.py" < "$SOURCE" > "$T/$NAME.ref"
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
    echo "Beta assembler register-label regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$NAME.stdout" ] && [ ! -s "$T/$NAME.stderr" ]
}

expect_assembly_reject() { # name status source
  NAME=$1 EXPECTED=$2 SOURCE=$3
  set +e
  "$ASM" < "$SOURCE" > "$T/$NAME.rejected" 2> "$T/$NAME.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "Beta assembler grammar regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$NAME.stderr" ]
}

printf '%s\n' \
  '        jmp r256x' \
  'r256x:' \
  '        jmp r0foo' \
  'r5x:' \
  '        imm r0, 252' \
  '        ret' \
  'r0foo:' \
  '        call r5x' \
  '        halt r0' > "$T/label-prefixes.beta"
expect_equal label-prefixes "$T/label-prefixes.beta"
expect_exit label-prefixes 252

printf '%s\n' \
  '        imm r0, 7' \
  '        imm r255, 245' \
  '        add r0, r255' \
  '        halt r0' > "$T/register-bounds.beta"
expect_equal register-bounds "$T/register-bounds.beta"
expect_exit register-bounds 252

# A quotient-based encoder accidentally treated high-bit words as signed and
# emitted only their low byte. Pin every byte against the independent assembler
# and observe the high byte after a word store.
printf '%s\n' \
  '        imm r0, 18446744073709551615' \
  '        imm r1, 1048576' \
  '        store r1, r0' \
  '        imm r2, 7' \
  '        add r1, r2' \
  '        loadb r0, r1' \
  '        halt r0' > "$T/full-word.beta"
expect_equal full-word "$T/full-word.beta"
expect_exit full-word 255

# The written width table and both encoders must retain the 11-byte
# opcode+register+register+address shape for conditional comparisons.
printf '%s\n' \
  '        jlt r0, r1, done' \
  '        jeq r0, r1, done' \
  'done:' \
  '        halt r0' > "$T/compare-width.beta"
expect_equal compare-width "$T/compare-width.beta"
[ "$(wc -c < "$T/compare-width.tape" | tr -d ' ')" -eq 24 ]
expect_exit compare-width 0

# Exercise the complete accepted IDENT alphabet and every documented db escape.
printf '%s\n' \
  '        jmp $._A0' \
  '$._A0:' \
  '        jmp done' \
  '        db "A B\n\t\r\0\\\""' \
  "        db \"\\'\"" \
  'done:' \
  '        imm r1, 00018446744073709551615' \
  '        imm r0, 0' \
  '        halt r0' > "$T/closed-valid.beta"
expect_equal closed-valid "$T/closed-valid.beta"
expect_exit closed-valid 0

# Canonical assembler operand errors use the existing status 7. The reference
# independently rejects the same out-of-range register grammar.
printf '%s\n' 'imm r256, 0' > "$T/register-over.beta"
set +e
"$ASM" < "$T/register-over.beta" > "$T/register-over.tape"
ASM_STATUS=$?
python3 "$GATE_DIR/asm_ref.py" < "$T/register-over.beta" \
  > "$T/register-over.ref" 2> "$T/register-over.stderr"
REF_STATUS=$?
set -e
[ "$ASM_STATUS" -eq 7 ] && [ "$REF_STATUS" -ne 0 ]

# Operand kinds and complete tokens are closed by the mnemonic width table.
printf '%s\n' 'halt 0' > "$T/halt-immediate.beta"
expect_assembly_reject halt-immediate 7 "$T/halt-immediate.beta"
printf '%s\n' 'imm r0, 1x' > "$T/decimal-junk.beta"
expect_assembly_reject decimal-junk 7 "$T/decimal-junk.beta"
printf '%s\n' 'imm r0, 18446744073709551616' > "$T/u64-overflow.beta"
expect_assembly_reject u64-overflow 7 "$T/u64-overflow.beta"
printf '%s\n' 'imm r0, 184467440737095516150' > "$T/u64-overflow-extra-digit.beta"
expect_assembly_reject u64-overflow-extra-digit 7 "$T/u64-overflow-extra-digit.beta"
printf '%s\n' 'halt' > "$T/missing-operand.beta"
expect_assembly_reject missing-operand 7 "$T/missing-operand.beta"

# IDENT definitions are nonempty, syntactically closed, and unique.
printf '%s\n' ':' > "$T/empty-label.beta"
expect_assembly_reject empty-label 7 "$T/empty-label.beta"
printf '%s\n' '1bad:' > "$T/invalid-label.beta"
expect_assembly_reject invalid-label 7 "$T/invalid-label.beta"
printf '%s\n' 'same:' 'same:' > "$T/duplicate-label.beta"
expect_assembly_reject duplicate-label 7 "$T/duplicate-label.beta"

# Mnemonics and db strings must consume exactly the documented lexical forms.
printf '%s\n' 'toolongxx r0' > "$T/long-mnemonic.beta"
expect_assembly_reject long-mnemonic 8 "$T/long-mnemonic.beta"
printf '%s\n' 'db junk "x"' > "$T/db-prefix.beta"
expect_assembly_reject db-prefix 9 "$T/db-prefix.beta"
printf '%s\n' 'db "x\q"' > "$T/db-escape.beta"
expect_assembly_reject db-escape 9 "$T/db-escape.beta"
printf 'db "a\001b"\n' > "$T/db-control.beta"
expect_assembly_reject db-control 9 "$T/db-control.beta"

# The outer source envelope is checked before tokenization, including comments.
# CR and LF both terminate comments; no other control byte is trivia.
printf 'jmp done; comment\rdone:\rhalt r0\r' > "$T/cr-comments.beta"
expect_equal cr-comments "$T/cr-comments.beta"
expect_exit cr-comments 0
printf '; comment\000\nhalt r0\n' > "$T/comment-nul.beta"
expect_assembly_reject comment-nul 9 "$T/comment-nul.beta"
printf 'halt\013r0\n' > "$T/vertical-tab.beta"
expect_assembly_reject vertical-tab 9 "$T/vertical-tab.beta"
printf '; comment\177\nhalt r0\n' > "$T/comment-del.beta"
expect_assembly_reject comment-del 9 "$T/comment-del.beta"
printf '; comment\303\251\nhalt r0\n' > "$T/comment-high-byte.beta"
expect_assembly_reject comment-high-byte 9 "$T/comment-high-byte.beta"

ELAPSED=$(python3 - "$START" <<'PY'
import sys, time
print(f"{time.time() - float(sys.argv[1]):.3f}")
PY
)
echo "Beta assembler regression: encoding and closed grammar cases passed (${ELAPSED}s, $(wc -c < "$T/strict-assembler.tape" | tr -d ' ')-byte candidate)"
