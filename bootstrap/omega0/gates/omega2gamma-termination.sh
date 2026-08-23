#!/usr/bin/env sh
# E2G TERMINATION CANARY — omega2gamma must TERMINATE on EVERY sample, supported or not.
#
# The translator is an untrusted state machine; on constructs outside its fragment it must refuse
# loudly (emit an E2G-UNSUPPORTED marker, which no downstream parser accepts) — never scan forever.
# Two real divergences motivated this gate: write_line with a non-literal argument, and a bare
# terminal expression (`{ 0 }`), both of which spun unguarded scans at end-of-input (one cost an
# 8h45m hung job). Every sample runs under a hard alarm; a timeout is a FAIL naming the sample.
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
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null 2>&1 ) || { echo "omega2gamma-termination FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe \
  < "${OMEGA_PATH_OMEGA0}/meaning/omega2gamma.beta" > "$T/e.asm" 2>/dev/null \
  && "$ASM" < "$T/e.asm" > "$T/e.tape" 2>/dev/null \
  && stamp_seed "$T/e.tape" "$SEED" "$T/omega2gamma.exe" >/dev/null 2>&1 \
  || { echo "omega2gamma-termination FAIL — build omega2gamma.beta"; exit 1; }

PASS=0; FAIL=0

# The retired block-form termination annotation used to make the translator
# consume the following machine-body brace as annotation syntax. It must now
# refuse explicitly rather than silently translating a different program.
retired=$(printf '%s\n' \
  'machine Main::main(&mut self)' \
  'terminates { decreases s -> Slice::Length; }' \
  '{ self.console.exit_process(0) }' \
  | "$T/omega2gamma.exe" 2>/dev/null)
case "$retired" in
  *E2G-UNSUPPORTED-terminates-clause*)
    PASS=$((PASS+1)); echo "  ok   retired terminates block : refused explicitly";;
  *)
    FAIL=$((FAIL+1)); echo "  FAIL retired terminates block : no explicit refusal";;
esac

for d in "${OMEGA_PATH_CORPUS}"/*/; do
  s=$(basename "$d")
  [ -f "$d/main.omg" ] || continue
  perl -e 'alarm 20; exec @ARGV' "$T/omega2gamma.exe" < "$d/main.omg" > /dev/null 2>&1
  rc=$?
  if [ "$rc" = 142 ] || [ "$rc" = 137 ]; then
    FAIL=$((FAIL+1)); echo "  FAIL $s : omega2gamma did not terminate (rc=$rc)"
  else
    PASS=$((PASS+1))
  fi
done
echo "omega2gamma termination canary (the translator halts on every sample, supported or refused): $PASS ok, $FAIL hung"
[ "$FAIL" = 0 ] && [ "$PASS" -gt 0 ]
