#!/usr/bin/env sh
# GAMMA MEANING DIAMOND — the independent reference evaluator (delta_ref.py) agrees with interp.gamma.
#
# interp.gamma is the current executable definition of what Delta programs mean,
# but its ADT / match / recursion evaluation needs an independent discriminator.
# delta_ref.py is that
# implementation; this gate runs random Delta programs (delta-fuzz-gen.py) through BOTH and asserts they
# agree on the printed result and exit code. A disagreement over ADTs / match / recursion / signed
# arithmetic / traps would expose a meaning bug in one of them. Deterministic; needs python3; skips cleanly.
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
. "$OMEGA_PATH_GAMMA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "delta diamond (py): skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'rm -rf -- "$T"' EXIT
stamp_gamma_compiler "$T/gc.exe" >/dev/null
"$T/gc.exe" < "$OMEGA_PATH_DELTA/interp.gamma" > "$T/g.tape" 2>/dev/null \
  && stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1 || { echo "delta diamond (py): interp build failed"; exit 1; }
G="$T/g.exe"

N=${1:-100}
PASS=0; FAIL=0

fixed_case() { # name source-file expected-status expected-output
  fixed_name=$1
  fixed_source=$2
  fixed_status=$3
  fixed_output=$4
  set +e
  fixed_ref=$(python3 delta_ref.py < "$fixed_source" 2>/dev/null)
  fixed_ref_status=$?
  fixed_oracle=$("$G" < "$fixed_source" 2>/dev/null)
  fixed_oracle_status=$?
  set -e
  if [ "$fixed_ref_status" = "$fixed_status" ] &&
     [ "$fixed_oracle_status" = "$fixed_status" ] &&
     [ "$fixed_ref" = "$fixed_output" ] &&
     [ "$fixed_oracle" = "$fixed_output" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $fixed_name: delta_ref=(out'$fixed_ref' rc$fixed_ref_status) interp=(out'$fixed_oracle' rc$fixed_oracle_status)"
  fi
}

printf '; before\r(+ 40 2)' > "$T/cr-comment.delta"
fixed_case cr-comment "$T/cr-comment.delta" 42 42
printf '; hidden\000\n(+ 40 2)' > "$T/comment-nul.delta"
fixed_case comment-nul "$T/comment-nul.delta" 255 ''
printf '(+ 40\0132)' > "$T/vertical-tab.delta"
fixed_case vertical-tab "$T/vertical-tab.delta" 255 ''
printf '; hidden\177\n(+ 40 2)' > "$T/comment-del.delta"
fixed_case comment-del "$T/comment-del.delta" 255 ''
printf '; hidden\303\251\n(+ 40 2)' > "$T/comment-high.delta"
fixed_case comment-high "$T/comment-high.delta" 255 ''

# Atom patterns beginning with lowercase are variable/catch-all patterns, not
# nullary constructors.  Keep this fixed fence alongside the generated corpus:
# the canonical Gamma interpreter has always implemented it.
printf '%s' '(match (Cons 1 Nil) (Nil 0) (other 42))' > "$T/catch-all.delta"
set +e
ro=$(python3 delta_ref.py < "$T/catch-all.delta" 2>/dev/null); rc=$?
io=$("$G" < "$T/catch-all.delta" 2>/dev/null); ic=$?
set -e
if [ "$rc" = "$ic" ] && [ "$ro" = "$io" ] && [ "$rc" = 42 ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1))
  echo "  FAIL catch-all pattern: delta_ref=(out'$ro' rc$rc) interp=(out'$io' rc$ic)"
fi

i=1
while [ "$i" -le "$N" ]; do
  s=$((660000 + i))
  python3 delta-fuzz-gen.py "$s" > "$T/p.delta"
  set +e
  ro=$(python3 delta_ref.py < "$T/p.delta" 2>/dev/null); rc=$?
  io=$("$G" < "$T/p.delta" 2>/dev/null); ic=$?
  set -e
  if [ "$rc" = "$ic" ] && [ "$ro" = "$io" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : delta_ref=(out'$ro' rc$rc) interp=(out'$io' rc$ic)"; sed 's/^/    /' "$T/p.delta"; fi
  i=$((i + 1))
done
echo "delta meaning diamond (independent delta_ref.py agrees with interp.gamma over $N random ADT/recursion programs): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
