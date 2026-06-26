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
# run NAME SOURCE.alp EXPECTED_EXIT
run() {
  EPS_ARCH=aarch64 "$BIN" "$2" "$T/out" >/dev/null 2>"$T/err" || {
    FAIL=$((FAIL+1)); echo "  FAIL $1 : compile/link/sign:"; sed 's/^/    /' "$T/err"; return; }
  set +e; "$T/out"; got=$?; set -e
  if [ "$got" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : exit $got, expected $3"; fi
}

# Slice 1: exit_process(<const>) -> the constant is the process exit status.
run "exit7 (exit_process(7))" samples/exit7.alp 7

echo "aarch64 macOS backend gate (slice 1): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
