#!/usr/bin/env sh
# INPUT-GRID MEANING TV — the summit's first INPUT-TAKING proofs.
#
# Closed samples prove one run; an input-taking sample's meaning is a FUNCTION of stdin. This gate
# instantiates that function over the sample's documented INPUT-GRID: each vector substitutes the
# omega2gamma entry's STDIN placeholder (substitution CLOSES the program, so the whole existing
# meaning-proof pipe applies unchanged per vector), the interpreter exit must match the grid's
# documented exit, and proof-kernel/check.beta must accept the encoder's claim with the perturbed control
# rejected. The grid rides in the sample header as `//   "VEC" -> EXIT` lines under INPUT-GRID.
# This is per-vector proof — the stepping stone toward symbolic (for-all-inputs) summit refinement.
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
command -v python3 >/dev/null 2>&1 || { echo "input-tv: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null 2>&1 ) || { echo "input-tv FAIL — bc build"; exit 1; }
b() { "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b omega2gamma.beta     "$T/omega2gamma.exe"    || { echo "input-tv FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "input-tv FAIL — build interp.beta"; exit 1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/check.beta  "$T/check.exe"  || { echo "input-tv FAIL — build check.beta"; exit 1; }

PASS=0; FAIL=0
tv() {
  src="${OMEGA_PATH_CORPUS}/$1/main.omg"
  "$T/omega2gamma.exe" < "$src" > "$T/g" 2>/dev/null
  grep -q 'STDIN' "$T/g" || { FAIL=$((FAIL+1)); echo "  FAIL $1 : no STDIN placeholder (not input-taking?)"; return; }
  grep -A100 'INPUT-GRID:' "$src" | grep -oE '"[^"]*" -> [0-9]+' | while IFS= read -r row; do
    vec=$(printf '%s' "$row" | sed 's/^"\(.*\)" -> .*/\1/')
    want=$(printf '%s' "$row" | grep -oE '[0-9]+$')
    list=$(python3 -c "
import sys
l = 'Nil'
for b in reversed('''$vec'''.encode()):
    l = '(Cons %d %s)' % (b, l)
print(l)")
    sed "s/STDIN/$list/" "$T/g" > "$T/gi"
    "$T/interp.exe" < "$T/gi" > "$T/iout" 2>&1; got=$?
    case "$(head -c 6 "$T/iout")" in '(Pair ')          # dual-channel: exit rides the printed pair
      got=$(head -1 "$T/iout" | sed 's/^(Pair \([0-9]*\) .*/\1/');; esac
    python3 gamma2claim.py < "$T/gi" > "$T/claims" 2>/dev/null || { echo "  FAIL $1 [\"$vec\"] : encoder refused"; exit 9; }
    l1=$(head -1 "$T/claims"); enc=${l1%% *}
    if [ "$enc" != "$got" ] || [ "$enc" != "$want" ]; then
      echo "  FAIL $1 [\"$vec\"] : exits disagree (encoder=$enc interp=$got documented=$want)"; exit 9; fi
    v=$(printf '%s' "${l1#* }" | "$T/check.exe")
    [ "$v" = accept ] || { echo "  FAIL $1 [\"$vec\"] : kernel rejected the claim"; exit 9; }
    v2=$(sed -n 2p "$T/claims" | "$T/check.exe")
    [ "$v2" = reject ] || { echo "  FAIL $1 [\"$vec\"] : perturbed claim NOT rejected"; exit 9; }
    render=$(grep '^#render ' "$T/claims" | sed 's/^#render //')
    if [ -n "$render" ]; then                           # the render pin: full stdout, byte-exact
      [ "$render" = "$(head -1 "$T/iout")" ] \
        || { echo "  FAIL $1 [\"$vec\"] : claimed structure differs from the interpreter's"; exit 9; }
    fi
  done
  if [ $? -eq 9 ]; then FAIL=$((FAIL+1)); return; fi
  n=$(grep -A100 'INPUT-GRID:' "$src" | grep -cE '"[^"]*" -> [0-9]+')
  PASS=$((PASS+1)); echo "  ok   $1 : $n input vectors, each exit kernel-PROVEN (perturbed rejected)"
}
tv stdin_checksum
tv stdin_upper
tv stdin_rot1
echo "input-grid meaning TV (input-taking samples proven per documented input vector): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] && [ "$PASS" -gt 0 ]
