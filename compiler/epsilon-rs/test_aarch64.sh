#!/usr/bin/env sh
# aarch64/macOS backend gate — the FIRST executable test of epsilon-rs on this
# platform. The x64 PE backend's output cannot run here; this backend emits ARM64
# assembly, clang assembles+links a Mach-O, codesign ad-hoc signs it, and we run
# the result and check its exit status. That closes the loop: epsilon is now a
# verifiable rung on macOS arm64, not just a compile-only one.
#
# Requires: clang + codesign (Xcode command line tools) and an arm64 host.
set -e
cd "$(dirname "$0")"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "aarch64 gate SKIP — not macOS arm64"; exit 0 ;;
esac
command -v clang     >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no clang"; exit 0; }
command -v codesign  >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no codesign"; exit 0; }

cargo build -q 2>/dev/null || { echo "aarch64 gate FAIL — cargo build"; exit 1; }
BIN=./target/debug/beta
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

PASS=0; FAIL=0
# compile NAME SOURCE.alp -> $T/out (binary), or record a failure
build() {
  EPS_ARCH=aarch64 "$BIN" "$2" "$T/out" >/dev/null 2>"$T/err" || {
    FAIL=$((FAIL+1)); echo "  FAIL $1 : compile/link/sign:"; sed 's/^/    /' "$T/err"; return 1; }
  return 0
}
# run NAME SOURCE.alp EXPECTED_EXIT  — value programs
run() {
  build "$1" "$2" || return
  set +e; "$T/out"; got=$?; set -e
  if [ "$got" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : exit $got, expected $3"; fi
}
# trap NAME SOURCE  — must die by signal (exit code > 128), not return a value
trap_test() {
  build "$1" "$2" || return
  set +e; "$T/out"; got=$?; set -e
  if [ "$got" -gt 128 ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : exit $got, expected a trap (>128)"; fi
}

# Slice 1: exit_process(<const>) -> the constant is the process exit status.
run "exit7 (exit_process(7))" samples/exit7.alp 7
# Slice 2: expressions + locals + arithmetic precedence.
run "arith (3 + 4*2)"          samples/arith.alp  11
run "locals (a=10; b=a-3; b*2)" samples/locals.alp 14
# Slices 4-5: transition/state control flow + back-edge loop + reassignment.
# (write_line is a no-op here; the exit code is fixed by the control flow: i 0->3.)
run "loop (count i 0->3, exit i)" samples/loop.alp 3
# Slice 6: machine calls (the DAG) — max(7,19)=19, max(19,12)=19, callee has states.
run "calls (max(max(7,19),12))" samples/calls.alp 19
# Slice 7a: data structs + mutable self fields — sum 1..=5 into self.total -> 15.
run "data (self fields, sum 1..5)" samples/data.alp 15
# Slice 2: the "trap everything" decision — overflow and /0 fault the process.
cat > "$T/ovf.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) { let x: i32 = 2000000000 + 2000000000; self.console.exit_process(x) }
EOF
trap_test "i32 add overflow traps" "$T/ovf.alp"
cat > "$T/dz.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) { let z: i32 = 0; let q: i32 = 5 / z; self.console.exit_process(q) }
EOF
trap_test "divide by zero traps" "$T/dz.alp"

echo "aarch64 macOS backend gate (slices 1-2, 4-6, 7a): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
