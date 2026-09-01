#!/usr/bin/env sh
# Lower-rooted structural obligations for one exact Alpha tape. The optional
# argument lets construction gates check any explicit tape before persistence.
set -eu

[ "$#" -le 1 ] || { echo "usage: $0 [ALPHA_TAPE]" >&2; exit 2; }

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ARTIFACT=${1:-"$OMEGA_PATH_GAMMA_COMPILER_TAPE"}
[ -f "$ARTIFACT" ] || { echo "missing Alpha tape: $ARTIFACT" >&2; exit 2; }
T=$(mktemp -d)
trap 'rm -rf -- "$T"' EXIT
materialize_beta_assembler "$T/assembler" >/dev/null
ASM="$T/assembler"

"$ASM" < "$GATE_DIR/gc-artifact-structure.beta" > "$T/artifact_structure_checker_bytecode.tape"
stamp_seed "$T/artifact_structure_checker_bytecode.tape" "$SEED" "$T/check" >/dev/null

PASS=0
FAIL=0
case_run() { # name expected-status input
  set +e
  "$T/check" < "$3" > "$T/stdout"
  got=$?
  set -e
  if [ "$got" = "$2" ] && [ ! -s "$T/stdout" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL $1: expected status $2/empty output, got $got/$(wc -c < "$T/stdout" | tr -d ' ') bytes"
  fi
}

case_run "persisted gc reachable CFG" 0 "$ARTIFACT"

# Jump-skipped data is valid and must not be decoded as instructions.
printf '%s\n' 'jmp code' 'db "not opcodes"' 'code:' 'imm r0, 0' 'halt r0' > "$T/inline.beta"
"$ASM" < "$T/inline.beta" > "$T/inline.tape"
case_run "unreachable inline data" 0 "$T/inline.tape"

: > "$T/empty"
case_run "empty tape" 1 "$T/empty"
dd if="$ARTIFACT" of="$T/truncated" bs=5 count=1 2>/dev/null
case_run "truncated reachable instruction" 1 "$T/truncated"

cp "$ARTIFACT" "$T/bad-opcode"
printf '\377' | dd of="$T/bad-opcode" bs=1 seek=0 conv=notrunc 2>/dev/null
case_run "unknown reachable opcode" 1 "$T/bad-opcode"

# Mutate a small direct-call fixture so the negative control does not depend on
# a particular instruction offset inside a future rebuilt gc artifact.
printf '%s\n' 'call f' 'halt r0' 'f:' 'ret' > "$T/target.beta"
"$ASM" < "$T/target.beta" > "$T/target.tape"
cp "$T/target.tape" "$T/interior-target"
printf '\001\000\000\000\000\000\000\000' > "$T/one.le64"
dd if="$T/one.le64" of="$T/interior-target" bs=1 seek=1 conv=notrunc 2>/dev/null
case_run "direct target into operand" 1 "$T/interior-target"

cp "$T/target.tape" "$T/range-target"
printf '\377\377\377\377\377\377\377\377' > "$T/max.le64"
dd if="$T/max.le64" of="$T/range-target" bs=1 seek=1 conv=notrunc 2>/dev/null
case_run "out-of-range direct target" 1 "$T/range-target"

# The whole-artifact checker also reconstructs procedure regions from direct
# call entries. The root must halt; a callee may return or terminate the whole
# run; and non-call control flow may not enter or leave a procedure region.
printf '%s\n' \
  'call f' \
  'imm r0, 0' \
  'halt r0' \
  'f:' \
  'jz r0, done' \
  'call f' \
  'done:' \
  'ret' > "$T/well-nested.beta"
"$ASM" < "$T/well-nested.beta" > "$T/well-nested.tape"
case_run "well-nested recursive call regions" 0 "$T/well-nested.tape"

# Region ids occupy one byte. The V2 Gamma procedure profile admits exactly the
# root plus 255 distinct callees; the next region must fail closed.
awk 'BEGIN {
  for (i = 1; i <= 255; i++) printf "call p%d\n", i
  print "imm r0, 0"
  print "halt r0"
  for (i = 1; i <= 255; i++) printf "p%d:\nret\n", i
}' > "$T/regions-256.beta"
"$ASM" < "$T/regions-256.beta" > "$T/regions-256.tape"
case_run "exact 256 procedure regions" 0 "$T/regions-256.tape"

awk 'BEGIN {
  for (i = 1; i <= 256; i++) printf "call p%d\n", i
  print "imm r0, 0"
  print "halt r0"
  for (i = 1; i <= 256; i++) printf "p%d:\nret\n", i
}' > "$T/regions-257.beta"
"$ASM" < "$T/regions-257.beta" > "$T/regions-257.tape"
case_run "adjacent 257th procedure region" 1 "$T/regions-257.tape"

printf '\024' > "$T/root-ret.tape"
case_run "root return without caller" 1 "$T/root-ret.tape"

printf '%s\n' \
  'call f' \
  'imm r0, 0' \
  'halt r0' \
  'f:' \
  'imm r0, 0' \
  'halt r0' > "$T/callee-halt.beta"
"$ASM" < "$T/callee-halt.beta" > "$T/callee-halt.tape"
case_run "callee whole-run halt" 0 "$T/callee-halt.tape"

printf '%s\n' \
  'call f' \
  'imm r0, 0' \
  'halt r0' \
  'f:' \
  'jmp f' > "$T/callee-cycle.beta"
"$ASM" < "$T/callee-cycle.beta" > "$T/callee-cycle.tape"
case_run "callee without return" 1 "$T/callee-cycle.tape"

printf '%s\n' \
  'call f' \
  'main_tail:' \
  'imm r0, 0' \
  'halt r0' \
  'f:' \
  'jmp main_tail' > "$T/cross-region.beta"
"$ASM" < "$T/cross-region.beta" > "$T/cross-region.tape"
case_run "non-call cross-region edge" 1 "$T/cross-region.tape"

# Pin the exact AlphaBootstrapV2 payload capacity.
dd if=/dev/zero of="$T/exact" bs=1048572 count=1 2>/dev/null
case_run "exact tape-hole payload" 0 "$T/exact"
cp "$T/exact" "$T/oversized"
printf '\000' >> "$T/oversized"
case_run "one byte over tape-hole payload" 253 "$T/oversized"

echo "gc artifact structural obligations: $PASS passed, $FAIL failed ($(wc -c < "$T/artifact_structure_checker_bytecode.tape" | tr -d ' ')-byte Alpha verifier tape)"
[ "$FAIL" = 0 ]
