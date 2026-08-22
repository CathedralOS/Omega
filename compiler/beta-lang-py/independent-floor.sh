#!/usr/bin/env sh
# INDEPENDENT FLOOR — the whole alpha->beta->bc floor has a complete second realization, in Python, and it
# runs real programs identically to the lineage. For each random program, build+run it TWO independent ways
# and require the same exit code and stdout:
#   python  : bc2.py (Beta -> asm) | asm_ref.py (asm -> tape) | alpha_ref.py (tape -> run)   [zero lineage binaries]
#   lineage : bc (Beta -> asm)     | assembler.alpha (asm -> tape) | the seed VM (tape -> run)
# Historical whole-floor comparison. Each stage is also covered by narrower
# reference checks (the legacy compiler comparison, ${OMEGA_PATH_BETA_ASSEMBLER}/asm-diamond.sh,
# ${OMEGA_PATH_ALPHA}/diamond-py.sh); this composes all three on a FRESH random corpus, so a divergence anywhere in the
# independent floor — compiler, assembler, or VM — shows up as a disagreement. It is the end-to-end
# This remains optional diagnostic tooling; it is not a trust or release gate.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "independent floor: skipped (python3 absent)"; exit 0; }
command -v cargo   >/dev/null 2>&1 || { echo "independent floor: skipped (no cargo for the on-ramp)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
BC="${OMEGA_PATH_BETA_RUST}"/build/bc.exe
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null 2>&1 ) || { echo "independent floor: bc build failed"; exit 1; }
[ -x "$BC" ] && [ -x "$ASM" ] || { echo "independent floor: skipped (bc/assembler missing)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
N=${1:-50}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((990000 + i))
  python3 beta-fuzz-gen.py "$s" > "$T/p.beta"
  # the ALL-PYTHON floor: bc2.py -> asm_ref.py -> alpha_ref.py (alpha_ref runs a raw tape file, no stamping)
  if python3 bc2.py < "$T/p.beta" > "$T/py.asm" 2>"$T/e" \
     && python3 "${OMEGA_PATH_BETA_ASSEMBLER}"/asm_ref.py < "$T/py.asm" > "$T/py.tape" 2>>"$T/e"; then
    po=$(python3 "${OMEGA_PATH_ALPHA}"/alpha_ref.py "$T/py.tape" </dev/null 2>/dev/null); pc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : python floor build error: $(cat "$T/e")"; i=$((i+1)); continue; fi
  # the ALL-LINEAGE floor: bc -> assembler -> stamped seed (reap SIGILL traps quietly)
  if "$BC" < "$T/p.beta" > "$T/ln.asm" 2>/dev/null && "$ASM" < "$T/ln.asm" > "$T/ln.tape" 2>/dev/null \
     && stamp_seed "$T/ln.tape" "$SEED" "$T/ln.exe" >/dev/null 2>&1; then
    lo=$(sh -c '"$1"; exit $?' _ "$T/ln.exe" </dev/null 2>/dev/null); lc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : lineage floor build error"; i=$((i+1)); continue; fi
  if [ "$pc" = "$lc" ] && [ "$po" = "$lo" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : python=(out='$po' rc=$pc)  lineage=(out='$lo' rc=$lc)"; sed 's/^/    /' "$T/p.beta"; fi
  i=$((i + 1))
done
echo "independent floor (bc2.py->asm_ref.py->alpha_ref.py == bc->assembler->seed over $N random programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
