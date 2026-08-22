#!/usr/bin/env sh
# GAMMA MEANING DIAMOND — the independent reference evaluator (gamma_ref.py) agrees with interp.beta.
#
# interp.beta is the canonical definition of what Gamma programs MEAN; the proof kernel proves theorems ABOUT that
# meaning, so its correctness underpins the proof edifice. Its arithmetic is cross-checked against the proof kernel's
# normalizer (seam-fuzz.sh) and it is checked on the omega samples (kernel-diamond) — but its ADT / match /
# recursion EVALUATION has no independent implementation to diamond against. gamma_ref.py is that
# implementation; this gate runs random Gamma programs (gamma-fuzz-gen.py) through BOTH and asserts they
# agree on the printed result and exit code. A disagreement over ADTs / match / recursion / signed
# arithmetic / traps would expose a meaning bug in one of them. Deterministic; needs python3; skips cleanly.
set -e
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
command -v python3 >/dev/null 2>&1 || { echo "gamma diamond (py): skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null 2>&1 ) || { echo "gamma diamond (py): bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < interp.beta > "$T/g.asm" 2>/dev/null && "$ASM" < "$T/g.asm" > "$T/g.tape" 2>/dev/null \
  && stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1 || { echo "gamma diamond (py): interp build failed"; exit 1; }
G="$T/g.exe"

N=${1:-100}
PASS=0; FAIL=0

# Atom patterns beginning with lowercase are variable/catch-all patterns, not
# nullary constructors.  Keep this fixed fence alongside the generated corpus:
# the canonical Beta interpreter has always implemented it and the ledger spike
# depends on it for fail-closed decoder arms.
printf '%s' '(match (Cons 1 Nil) (Nil 0) (other 42))' > "$T/catch-all.gamma"
set +e
ro=$(python3 gamma_ref.py < "$T/catch-all.gamma" 2>/dev/null); rc=$?
io=$("$G" < "$T/catch-all.gamma" 2>/dev/null); ic=$?
set -e
if [ "$rc" = "$ic" ] && [ "$ro" = "$io" ] && [ "$rc" = 42 ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL catch-all pattern: gamma_ref=(out'$ro' rc$rc) interp=(out'$io' rc$ic)"
fi

i=1
while [ "$i" -le "$N" ]; do
  s=$((660000 + i))
  python3 gamma-fuzz-gen.py "$s" > "$T/p.gamma"
  set +e
  ro=$(python3 gamma_ref.py < "$T/p.gamma" 2>/dev/null); rc=$?
  io=$("$G" < "$T/p.gamma" 2>/dev/null); ic=$?
  set -e
  if [ "$rc" = "$ic" ] && [ "$ro" = "$io" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : gamma_ref=(out'$ro' rc$rc) interp=(out'$io' rc$ic)"; sed 's/^/    /' "$T/p.gamma"; fi
  i=$((i + 1))
done
echo "gamma meaning diamond (independent gamma_ref.py agrees with interp.beta over $N random ADT/recursion programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
