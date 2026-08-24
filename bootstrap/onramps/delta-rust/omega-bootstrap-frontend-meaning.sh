#!/usr/bin/env sh
# O0/O1 FRONTEND MEANING — run the Delta-written frontend through the
# Beta-written omega2gamma elaborator and canonical Gamma interpreter.
set -e
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "omega-bootstrap frontend meaning: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$GATE_DIR"
SAMPLES="$OMEGA_PATH_DELTA/samples"
. "$OMEGA_PATH_BETA/artifact_env.sh"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

command -v python3 >/dev/null 2>&1 || { echo "omega-bootstrap frontend meaning: python3 required" >&2; exit 2; }
command -v perl >/dev/null 2>&1 || { echo "omega-bootstrap frontend meaning: perl required for the hard elaboration timeout" >&2; exit 2; }
stamp_beta_compiler "$T/bc.exe" >/dev/null \
  || { echo "omega-bootstrap frontend meaning FAIL — Beta compiler artifact"; exit 1; }

build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" \
  || { echo "omega-bootstrap frontend meaning FAIL — omega2gamma build"; exit 1; }
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" \
  || { echo "omega-bootstrap frontend meaning FAIL — Gamma interpreter build"; exit 1; }

FRONTEND="$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-frontend.alp"
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" < "$FRONTEND" > "$T/frontend.gamma"
[ -s "$T/frontend.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/frontend.gamma" \
  || { echo "omega-bootstrap frontend meaning FAIL — frontend elaboration unsupported"; exit 1; }
frontend_gamma_bytes=$(wc -c < "$T/frontend.gamma" | tr -d ' ')
[ "$frontend_gamma_bytes" -le 1048576 ] \
  || { echo "omega-bootstrap frontend meaning FAIL — frontend Gamma expanded to $frontend_gamma_bytes bytes"; exit 1; }

bundle_program() {
  source=$1
  destination=$2
  python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
    main.omg="$source" > "$T/input.bundle"
  bytes=$(od -An -tu1 < "$T/input.bundle" | tr ' ' '\n' | grep -vE '^$' | tr '\n' ' ')
  reverse=""
  for byte in $bytes; do reverse="$byte $reverse"; done
  list=Nil
  for byte in $reverse; do list="(Cons $byte $list)"; done
  sed "s/STDIN/$list/" "$T/frontend.gamma" > "$destination"
}

PASS=0
FAIL=0
run_gamma() {
  label=$1
  program=$2
  expected=$3
  mode=${4:-scalar}
  set +e
  "$T/interp.exe" < "$program" > "$T/result"
  process_exit=$?
  set -e
  got=$process_exit
  payload_ok=1
  case "$mode" in
    pair-nonempty)
      got=$(sed -n 's/^(Pair \([0-9][0-9]*\) .*/\1/p' "$T/result")
      grep -q '(Cons ' "$T/result" || payload_ok=0
      ;;
    pair-empty)
      got=$(sed -n 's/^(Pair \([0-9][0-9]*\) .*/\1/p' "$T/result")
      grep -qx "(Pair $expected Nil)" "$T/result" || payload_ok=0
      ;;
    scalar)
      printf '%s\n' "$expected" > "$T/expected"
      cmp -s "$T/expected" "$T/result" || payload_ok=0
      ;;
  esac
  if [ "$got" = "$expected" ] && [ "$payload_ok" = 1 ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL $label: observed $got/$mode, expected $expected; output $(head -c 240 "$T/result" | tr '\n' ' ')"
  fi
}

bundle_program "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" "$T/canonical.gamma"
run_gamma "canonical cli_mvp retained-operand digest" "$T/canonical.gamma" 107 pair-nonempty

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(7);}' > "$T/zero-write.omg"
bundle_program "$T/zero-write.omg" "$T/zero-write.gamma"
run_gamma "O1 zero-write body" "$T/zero-write.gamma" 7 pair-nonempty

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("A");self.console.write_line("BC");self.console.exit_process(3);}' > "$T/two-write.omg"
bundle_program "$T/two-write.omg" "$T/two-write.gamma"
run_gamma "O1 two ordered writes" "$T/two-write.gamma" 201 pair-nonempty

# Teeth: a syntactically valid bundle whose source names an operation outside O0
# must follow the frontend's semantic rejection path, not merely execute safely.
sed 's/std::console/std::other/' "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" > "$T/reject.omg"
bundle_program "$T/reject.omg" "$T/reject.gamma"
run_gamma "unknown import semantic rejection" "$T/reject.gamma" 251 pair-empty

# Pin both branches of the private method-state ABI used by the frontend:
# a void call and a value-returning call must preserve all threaded self slots.
"$T/elaborate.exe" < "$SAMPLES/omega-bootstrap-meaning-methods.alp" > "$T/methods.gamma"
if [ -s "$T/methods.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/methods.gamma"; then
  run_gamma "multi-slot void/value method threading" "$T/methods.gamma" 26
else
  FAIL=$((FAIL + 1))
  echo "  FAIL multi-slot void/value method threading: unsupported elaboration"
fi

echo "omega-bootstrap frontend meaning (omega2gamma.beta -> interp.beta): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
