#!/usr/bin/env sh
# DISCHARGE SOUNDNESS — a diamond between the two contract paths. The compiler discharges an
# `ensures` STATICALLY (contracts.sh: a proof, for all inputs, that the trust anchor checks)
# AND desugars it to a RUNTIME assert that traps if violated for the actual input. If static
# discharge is sound, the runtime assert can never trip on a discharged contract. This runs the
# discharged machines across many inputs and confirms zero traps -- static "holds for all
# inputs" and dynamic "never traps" agreeing. A negative control (a contract the compiler does
# NOT discharge, false for every input) confirms the runtime layer is real: it DOES trap.
#
# Needs the aarch64/macOS run path; skips cleanly elsewhere.
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "discharge-soundness SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "discharge-soundness SKIP — no $t"; exit 0; }; done

cargo build -q 2>/dev/null || { echo "discharge-soundness FAIL — cargo build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
PASS=0; FAIL=0

# 1. the discharged machines must NEVER trap, on any input -> the driver always exits 42.
EPS_ARCH=aarch64 ./target/debug/beta samples/contract-driver.alp "$T/drv" >/dev/null 2>"$T/e" \
  || { echo "discharge-soundness FAIL — compiling contract-driver:"; sed 's/^/    /' "$T/e"; exit 1; }
for n in 0 1 2 7 42 63 100 127 128 200 254 255; do
  set +e; printf "\\$(printf '%03o' "$n")" | "$T/drv"; got=$?; set -e
  if [ "$got" = 42 ]; then PASS=$((PASS + 1)); else
    FAIL=$((FAIL + 1)); echo "  FAIL n=$n : exit $got (a discharged contract trapped at runtime!)"; fi
done

# 2. negative control: a contract the compiler does NOT discharge, false for every input, must
# TRAP at runtime -- so the no-trap result above is meaningful, not a vacuous/disabled check.
cat > "$T/neg.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; }
data Main { console: Console; }
machine impossible(a: i32) -> i32 ensures result > a + 5 { return a; }
machine Main::main(&mut self) { let n: i32 = read_byte(); let r: i32 = impossible(n); self.console.exit_process(42) }
EOF
EPS_ARCH=aarch64 ./target/debug/beta "$T/neg.alp" "$T/neg" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling negative control"; exit 1; }
set +e; ( printf '\007' | "$T/neg" ) >/dev/null 2>&1; got=$?; set -e
if [ "$got" -gt 128 ]; then PASS=$((PASS + 1)); echo "  ok   negative control traps at runtime (exit $got)";
else FAIL=$((FAIL + 1)); echo "  FAIL negative control did NOT trap (exit $got)"; fi

echo "discharge soundness (static proof and runtime assert agree on discharged contracts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
