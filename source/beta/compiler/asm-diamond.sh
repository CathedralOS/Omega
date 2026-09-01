#!/usr/bin/env sh
# ASSEMBLER DIAMOND — the independent reference assembler (asm_ref.py) agrees with the real one.
#
# asm_ref.py is an independent realization (Python, written from the encoding,
# not ported from assembler.beta). This gate compares both assemblers over a
# corpus to catch encoding regressions. Agreement is diagnostic evidence, not
# source-to-artifact authority; asm_ref.py is untrusted and runtime never runs it.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "asm-diamond SKIP — no python3"; exit 0; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
materialize_beta_assembler "$T/assembler" >/dev/null
ASM="$T/assembler"
PASS=0; FAIL=0

cmp_asm() {  # name  asmfile
  "$ASM" < "$2" > "$T/real.tape" 2>/dev/null
  if ! python3 asm_ref.py < "$2" > "$T/ref.tape" 2>"$T/e"; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : asm_ref.py error: $(cat "$T/e")"; return; fi
  if cmp -s "$T/real.tape" "$T/ref.tape"; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : tapes DIFFER (real=$(wc -c <"$T/real.tape"|tr -d ' ') ref=$(wc -c <"$T/ref.tape"|tr -d ' '))"; fi
}

# the assembler's own source (the ultimate case: assemble the real assembler both ways) + its examples
cmp_asm "assembler.beta (self)" "assembler.beta"
for ex in examples/*.beta; do [ -f "$ex" ] && cmp_asm "example $(basename "$ex")" "$ex"; done

echo "assembler diamond (independent reference asm_ref.py assembles byte-identically to assembler.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
