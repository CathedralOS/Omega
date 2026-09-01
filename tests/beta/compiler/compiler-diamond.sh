#!/usr/bin/env sh
# COMPILER DIAMOND — the independent reference assembler (beta_ref.py) agrees with the real one.
#
# beta_ref.py is an independent realization (Python, written from the encoding,
# not ported from beta_compiler.beta). This gate compares both compilers over a
# corpus to catch encoding regressions. Agreement is diagnostic evidence, not
# source-to-artifact authority; beta_ref.py is untrusted and runtime never runs it.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "compiler-diamond SKIP — no python3"; exit 0; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
materialize_beta_compiler "$T/assembler" >/dev/null
ASM="$T/assembler"
PASS=0; FAIL=0

cmp_asm() {  # name  asmfile
  "$ASM" < "$2" > "$T/real.tape" 2>/dev/null
  if ! python3 beta_ref.py < "$2" > "$T/ref.tape" 2>"$T/e"; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : beta_ref.py error: $(cat "$T/e")"; return; fi
  if cmp -s "$T/real.tape" "$T/ref.tape"; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : tapes DIFFER (real=$(wc -c <"$T/real.tape"|tr -d ' ') ref=$(wc -c <"$T/ref.tape"|tr -d ' '))"; fi
}

# the compiler's own source (the ultimate case: assemble the real assembler both ways) + its examples
cmp_asm "beta_compiler.beta (self)" "$OMEGA_PATH_BETA_COMPILER_SOURCE"
for ex in examples/*.beta; do [ -f "$ex" ] && cmp_asm "example $(basename "$ex")" "$ex"; done

echo "compiler diamond (independent reference beta_ref.py assembles byte-identically to beta_compiler.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
