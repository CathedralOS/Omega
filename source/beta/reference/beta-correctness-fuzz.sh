#!/usr/bin/env sh
# BETA COMPILER CORRECTNESS FUZZ — random differential testing of bc's SEMANTICS (not just its
# reproducibility or cross-compiler agreement). For each random program, run it two
# independent ways and require agreement on exit code AND stdout:
#   interpret : beta_interp.py runs the Beta source directly (a second, independent definition of Beta)
#   compile   : bc compiles it to Alpha bytecode, which the seed VM runs
# A disagreement means bc miscompiled the program (or the interpreter is wrong — either way, a loud signal
# worth investigating, never a silent pass). Both are UNTRUSTED and checked against each other.
# Deterministic (fixed base seed). Needs python3 plus the persisted lattice bc
# artifact and assembler; skips cleanly when the host cannot run them.
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
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "beta correctness fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_BETA_COMPILER}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'trash "$T"' EXIT
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "beta correctness fuzz: lattice bc artifact unavailable"; exit 1; }
N=${1:-60}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((770000 + i))
  python3 beta-fuzz-gen.py "$s" > "$T/p.beta"
  # interpret directly
  io=$(python3 beta_interp.py "$T/p.beta" </dev/null 2>/dev/null); ic=$?
  # compile with bc, run on the seed VM (reap SIGILL traps quietly, propagate the 132 exit)
  if "$BC" < "$T/p.beta" > "$T/p.tape" 2>/dev/null \
     && stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1; then
    co=$(sh -c '"$1"; exit $?' _ "$T/p.exe" </dev/null 2>/dev/null); cc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : Beta compiler could not build the program"; i=$((i+1)); continue
  fi
  if [ "$ic" = "$cc" ] && [ "$io" = "$co" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1))
    echo "  FAIL seed=$s : interpret=(out='$io' rc=$ic)  compiled=(out='$co' rc=$cc)"
    sed 's/^/    /' "$T/p.beta"
  fi
  i=$((i + 1))
done
echo "beta compiler correctness fuzz (interpret == compile+run over $N random programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
