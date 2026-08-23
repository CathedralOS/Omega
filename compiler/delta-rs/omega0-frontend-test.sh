#!/usr/bin/env sh
# Focused native acceptance gate for the Delta-written O0 front end.
set -e
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "omega0 frontend: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$GATE_DIR"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "omega0 frontend: native gate skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v cargo >/dev/null 2>&1 || { echo "omega0 frontend: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "omega0 frontend: skipped (python3 absent)"; exit 0; }
command -v clang >/dev/null 2>&1 || { echo "omega0 frontend: skipped (clang absent)"; exit 0; }
command -v codesign >/dev/null 2>&1 || { echo "omega0 frontend: skipped (codesign absent)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cargo build -q
DELTA_ARCH=aarch64 ./target/debug/delta samples/omega0-frontend.alp "$T/frontend" >/dev/null

PASS=0
FAIL=0
bundle_one() {
  python3 "$OMEGA_PATH_OMEGA0/compiler/omega0_bundle.py" pack main.omg="$1" > "$2"
}
run_bundle() {
  label=$1 input=$2 expected=$3
  set +e
  "$T/frontend" < "$input" > /dev/null 2>&1
  got=$?
  set -e
  if [ "$got" = "$expected" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $label: exit $got, expected $expected"
  fi
}
run_source() {
  label=$1 source=$2 expected=$3
  bundle_one "$source" "$T/case.bundle"
  run_bundle "$label" "$T/case.bundle" "$expected"
}

run_source "canonical cli_mvp" "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" 107

printf 'use omega::language::std::console; // import\ndata Main{console:Console;}machine Main::main(&mut self){self.console.write_line("A\\n");self.console.exit_process(2);}' > "$T/variant.omg"
run_source "trivia, cooked escape, no final newline" "$T/variant.omg" 77

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("\303\251");self.console.exit_process(3);}' > "$T/utf8.omg"
run_source "UTF-8 string payload" "$T/utf8.omg" 116

reject_source() {
  label=$1 body=$2
  printf '%s' "$body" > "$T/reject.omg"
  run_source "$label" "$T/reject.omg" 251
}

reject_source "unknown import" 'use omega::language::std::other; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "duplicate declaration" 'use omega::language::std::console; data Main{console:Console;} data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "missing entry" 'use omega::language::std::console; data Main{console:Console;}'
reject_source "wrong field type" 'use omega::language::std::console; data Main{console:Main;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "wrong machine receiver" 'use omega::language::std::console; data Main{console:Console;} machine Console::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "unknown operation" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write("x");self.console.exit_process(0);}'
reject_source "write_line missing argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line();self.console.exit_process(0);}'
reject_source "write_line extra argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x","y");self.console.exit_process(0);}'
reject_source "write_line wrong argument type" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line(1);self.console.exit_process(0);}'
reject_source "exit_process missing argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process();}'
reject_source "exit_process extra argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0,1);}'
reject_source "exit_process wrong argument type" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process("0");}'
reject_source "reversed effects" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(0);self.console.write_line("x");}'
reject_source "trailing construct" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);} data Extra{}'
reject_source "unterminated string" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x);self.console.exit_process(0);}'
reject_source "invalid escape" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("\q");self.console.exit_process(0);}'
reject_source "i32 literal overflow" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(2147483648);}'

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("' > "$T/invalid-utf8.omg"
printf '\377' >> "$T/invalid-utf8.omg"
printf '");self.console.exit_process(0);}' >> "$T/invalid-utf8.omg"
run_source "invalid UTF-8" "$T/invalid-utf8.omg" 251

python3 "$OMEGA_PATH_OMEGA0/compiler/omega0_bundle.py" pack \
  a.omg="$T/variant.omg" b.omg="$T/variant.omg" > "$T/multi.bundle"
run_bundle "multiple sources outside O0" "$T/multi.bundle" 251

dd if=/dev/zero of="$T/oversize.omg" bs=2049 count=1 2>/dev/null
run_source "checked source exhaustion" "$T/oversize.omg" 252

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("' > "$T/text-exhaust.omg"
dd if=/dev/zero bs=1 count=1025 2>/dev/null | tr '\000' x >> "$T/text-exhaust.omg"
printf '");self.console.exit_process(0);}' >> "$T/text-exhaust.omg"
run_source "checked decoded-string exhaustion" "$T/text-exhaust.omg" 252

cp "$T/case.bundle" "$T/trailing.bundle"; printf x >> "$T/trailing.bundle"
run_bundle "bundle trailing byte" "$T/trailing.bundle" 251

# The reference compiler is only an on-ramp. Compile the frontend once more
# through the Delta-written lowermachine and require the resulting program to
# preserve both an accepted observation and a semantic rejection.
DELTA_ARCH=aarch64 ./target/debug/delta samples/lowermachine.alp "$T/lowermachine" >/dev/null
"$T/lowermachine" < samples/omega0-frontend.alp > "$T/frontend-self.s"
clang -arch arm64 -o "$T/frontend-self" "$T/frontend-self.s"
codesign -f -s - "$T/frontend-self" >/dev/null 2>&1
bundle_one "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" "$T/canonical.bundle"
set +e
"$T/frontend-self" < "$T/canonical.bundle" > /dev/null 2>&1; self_ok=$?
"$T/frontend-self" < "$T/multi.bundle" > /dev/null 2>&1; self_bad=$?
set -e
if [ "$self_ok" = 107 ] && [ "$self_bad" = 251 ]; then
  PASS=$((PASS+2))
else
  FAIL=$((FAIL+1)); echo "  FAIL Delta-written self-host path: accepted=$self_ok rejected=$self_bad"
fi

echo "omega0 Delta frontend: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
