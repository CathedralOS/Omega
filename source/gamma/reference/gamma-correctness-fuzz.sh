#!/usr/bin/env sh
# GAMMA COMPILER CORRECTNESS FUZZ — random differential testing of the Gamma compiler's semantics (not just its
# reproducibility or cross-compiler agreement). For each random program, run it two
# independent ways and require agreement on exit code AND stdout:
#   interpret : gamma_interp.py runs the Gamma source directly (a second, independent definition of Gamma)
#   compile   : gc compiles it to Alpha bytecode, which the seed VM runs
# A disagreement means the Gamma compiler miscompiled the program (or the interpreter is wrong — either way, a loud signal
# worth investigating, never a silent pass). Both are UNTRUSTED and checked against each other.
# Deterministic (fixed base seed). Needs python3 plus the persisted Gamma compiler
# artifact; skips cleanly when the host cannot run it.
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
command -v python3 >/dev/null 2>&1 || { echo "gamma correctness fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_GAMMA_COMPILER}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'rm -rf -- "$T"' EXIT
GC="$T/gc.exe"
stamp_gamma_compiler "$GC" >/dev/null 2>&1 || { echo "gamma correctness fuzz: lattice Gamma compiler artifact unavailable"; exit 1; }
N=${1:-60}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((770000 + i))
  python3 gamma-fuzz-gen.py "$s" > "$T/p.gamma"
  # interpret directly
  io=$(python3 gamma_interp.py "$T/p.gamma" </dev/null 2>/dev/null); ic=$?
  # compile with Gamma, run on the seed VM (reap SIGILL traps quietly, propagate the 132 exit)
  if "$GC" < "$T/p.gamma" > "$T/p.tape" 2>/dev/null \
     && stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1; then
    co=$(sh -c '"$1"; exit $?' _ "$T/p.exe" </dev/null 2>/dev/null); cc=$?
  else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : Gamma compiler could not build the program"; i=$((i+1)); continue
  fi
  if [ "$ic" = "$cc" ] && [ "$io" = "$co" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1))
    echo "  FAIL seed=$s : interpret=(out='$io' rc=$ic)  compiled=(out='$co' rc=$cc)"
    sed 's/^/    /' "$T/p.gamma"
  fi
  i=$((i + 1))
done
echo "gamma compiler correctness fuzz (interpret == compile+run over $N random programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
