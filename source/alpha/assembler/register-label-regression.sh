#!/usr/bin/env sh
# Focused lexical regression: labels beginning r+digit are not registers.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$GATE_DIR"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Alpha assembler register-label regression: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "Alpha assembler register-label regression: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'trash "$T"' EXIT HUP INT TERM
START=$(python3 -c 'import time; print(time.time())')
BOOTSTRAP_ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"

# Exercise the checked-in Alpha source, not merely the previously stamped
# assembler artifact. The old assembler is sufficient to construct this fresh
# candidate; every case below runs the fresh candidate in the Alpha seed.
"$BOOTSTRAP_ASM" < "$OMEGA_PATH_ALPHA_ASSEMBLER/assembler.alpha" \
  > "$T/strict-assembler.tape"
stamp_seed "$T/strict-assembler.tape" "$SEED" "$T/strict-assembler" >/dev/null
ASM="$T/strict-assembler"

expect_equal() {
  NAME=$1 SOURCE=$2
  "$ASM" < "$SOURCE" > "$T/$NAME.tape"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" < "$SOURCE" > "$T/$NAME.ref"
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
    echo "Alpha assembler register-label regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
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
    echo "Alpha assembler grammar regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
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
  '        halt r0' > "$T/label-prefixes.alpha"
expect_equal label-prefixes "$T/label-prefixes.alpha"
expect_exit label-prefixes 252

printf '%s\n' \
  '        imm r0, 7' \
  '        imm r255, 245' \
  '        add r0, r255' \
  '        halt r0' > "$T/register-bounds.alpha"
expect_equal register-bounds "$T/register-bounds.alpha"
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
  '        halt r0' > "$T/full-word.alpha"
expect_equal full-word "$T/full-word.alpha"
expect_exit full-word 255

# The written width table and both encoders must retain the 11-byte
# opcode+register+register+address shape for conditional comparisons.
printf '%s\n' \
  '        jlt r0, r1, done' \
  '        jeq r0, r1, done' \
  'done:' \
  '        halt r0' > "$T/compare-width.alpha"
expect_equal compare-width "$T/compare-width.alpha"
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
  '        halt r0' > "$T/closed-valid.alpha"
expect_equal closed-valid "$T/closed-valid.alpha"
expect_exit closed-valid 0

# Canonical assembler operand errors use the existing status 7. The reference
# independently rejects the same out-of-range register grammar.
printf '%s\n' 'imm r256, 0' > "$T/register-over.alpha"
set +e
"$ASM" < "$T/register-over.alpha" > "$T/register-over.tape"
ASM_STATUS=$?
python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" < "$T/register-over.alpha" \
  > "$T/register-over.ref" 2> "$T/register-over.stderr"
REF_STATUS=$?
set -e
[ "$ASM_STATUS" -eq 7 ] && [ "$REF_STATUS" -ne 0 ]

# Operand kinds and complete tokens are closed by the mnemonic width table.
printf '%s\n' 'halt 0' > "$T/halt-immediate.alpha"
expect_assembly_reject halt-immediate 7 "$T/halt-immediate.alpha"
printf '%s\n' 'imm r0, 1x' > "$T/decimal-junk.alpha"
expect_assembly_reject decimal-junk 7 "$T/decimal-junk.alpha"
printf '%s\n' 'imm r0, 18446744073709551616' > "$T/u64-overflow.alpha"
expect_assembly_reject u64-overflow 7 "$T/u64-overflow.alpha"
printf '%s\n' 'imm r0, 184467440737095516150' > "$T/u64-overflow-extra-digit.alpha"
expect_assembly_reject u64-overflow-extra-digit 7 "$T/u64-overflow-extra-digit.alpha"
printf '%s\n' 'halt' > "$T/missing-operand.alpha"
expect_assembly_reject missing-operand 7 "$T/missing-operand.alpha"

# IDENT definitions are nonempty, syntactically closed, and unique.
printf '%s\n' ':' > "$T/empty-label.alpha"
expect_assembly_reject empty-label 7 "$T/empty-label.alpha"
printf '%s\n' '1bad:' > "$T/invalid-label.alpha"
expect_assembly_reject invalid-label 7 "$T/invalid-label.alpha"
printf '%s\n' 'same:' 'same:' > "$T/duplicate-label.alpha"
expect_assembly_reject duplicate-label 7 "$T/duplicate-label.alpha"

# Mnemonics and db strings must consume exactly the documented lexical forms.
printf '%s\n' 'toolongxx r0' > "$T/long-mnemonic.alpha"
expect_assembly_reject long-mnemonic 8 "$T/long-mnemonic.alpha"
printf '%s\n' 'db junk "x"' > "$T/db-prefix.alpha"
expect_assembly_reject db-prefix 9 "$T/db-prefix.alpha"
printf '%s\n' 'db "x\q"' > "$T/db-escape.alpha"
expect_assembly_reject db-escape 9 "$T/db-escape.alpha"
printf 'db "a\001b"\n' > "$T/db-control.alpha"
expect_assembly_reject db-control 9 "$T/db-control.alpha"

ELAPSED=$(python3 - "$START" <<'PY'
import sys, time
print(f"{time.time() - float(sys.argv[1]):.3f}")
PY
)
echo "Alpha assembler regression: encoding and closed grammar cases passed (${ELAPSED}s, $(wc -c < "$T/strict-assembler.tape" | tr -d ' ')-byte candidate)"
