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

# 1b. SIGN regression: epsilon `i32` is SIGNED, but discharge proofs are over delta NATURALS.
# Run the discharged machines on a NEGATIVE input too -- a discharge unsound for negatives
# (e.g. a parameter-witness `result <= a + b` with b < 0) would trap here while having been
# "proved". The compiler now refuses unprovably-non-negative witnesses; this guards it.
cat > "$T/negin.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine succ_gt(a: i32) -> i32 ensures result > a { return a + 1; }
machine within10(a: i32) -> i32 ensures result <= a + 10 { return a; }
machine azr(a: i32) -> i32 ensures result == a { return a + 0; }
machine Main::main(&mut self) {
    let n: i32 = 0 - 50;
    let g: i32 = succ_gt(n);
    let w: i32 = within10(n);
    let z: i32 = azr(n);
    self.console.exit_process(42)
}
EOF
EPS_ARCH=aarch64 ./target/debug/beta "$T/negin.alp" "$T/negin" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling negative-input driver"; exit 1; }
set +e; "$T/negin"; got=$?; set -e
if [ "$got" = 42 ]; then PASS=$((PASS + 1)); echo "  ok   discharged contracts hold on NEGATIVE input (signed i32)";
else FAIL=$((FAIL + 1)); echo "  FAIL discharged contract trapped on negative input (exit $got) -- sign-unsoundness"; fi

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

# 3. CALL-SITE COMPOSITION soundness: a wrapper's precondition is statically proved to imply its
# callee's (forwarding + weakening). At runtime the callee still asserts its own precondition --
# so if the wrapper is called with arguments that satisfy ITS precondition, the callee's assert
# must never trap. (An unsound composition discharge would let it.) Each wrapper is called with
# an argument constructed to satisfy its own precondition across the whole input range.
cat > "$T/comp.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; }
data Main { console: Console; }
machine needs_pos(x: i32) -> i32 requires x >= 1 { return x; }
machine fwd(a: i32) -> i32 requires a >= 1 { return needs_pos(a); }
machine weaken(a: i32) -> i32 requires a >= 3 { return needs_pos(a); }
machine Main::main(&mut self) {
    let n: i32 = read_byte();      // 0..255
    let f: i32 = fwd(n + 1);       // arg >= 1 -> satisfies fwd's requires; forwarding => needs_pos ok
    let w: i32 = weaken(n + 3);    // arg >= 3 -> satisfies weaken's requires; weakening => needs_pos ok
    self.console.exit_process(42)
}
EOF
EPS_ARCH=aarch64 ./target/debug/beta "$T/comp.alp" "$T/comp" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling composition driver"; exit 1; }
for n in 0 1 5 42 100 200 252; do
  set +e; printf "\\$(printf '%03o' "$n")" | "$T/comp"; got=$?; set -e
  if [ "$got" = 42 ]; then PASS=$((PASS + 1)); else
    FAIL=$((FAIL + 1)); echo "  FAIL composition n=$n : exit $got (callee precondition trapped though the wrapper's held!)"; fi
done

# 4. PARAMETER-GAP postcondition soundness: `within_b` (ensures result <= a + b) is discharged ONLY
# under `requires b >= 0`, and CONDITIONALLY on that fact. At runtime the entry assert of the requires
# guards the postcondition: with b >= 0 the postcondition `7 <= 7 + b` must never trap, and with b < 0
# the REQUIRES assert (not the postcondition) must trap -- so the signed witness is sound exactly where
# the conditional cert claims, and unsound input is caught at the entry, not silently "proved".
cat > "$T/pgap.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; }
data Main { console: Console; }
machine within_b(a: i32, b: i32) -> i32 requires b >= 0 ensures result <= a + b { return a; }
machine atleast_b(a: i32, b: i32) -> i32 requires b >= 0 ensures result >= a { return a + b; }
machine Main::main(&mut self) {
    let n: i32 = read_byte();        // 0..255, so n >= 0 satisfies requires b >= 0
    let w: i32 = within_b(7, n);     // postcondition 7 <= 7 + n holds for n >= 0 -> no trap
    let g: i32 = atleast_b(7, n);    // Ge twin: 7 + n >= 7 holds for n >= 0 -> no trap
    self.console.exit_process(42)
}
EOF
EPS_ARCH=aarch64 ./target/debug/beta "$T/pgap.alp" "$T/pgap" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling parameter-gap driver"; exit 1; }
for n in 0 1 5 42 100 200 255; do
  set +e; printf "\\$(printf '%03o' "$n")" | "$T/pgap"; got=$?; set -e
  if [ "$got" = 42 ]; then PASS=$((PASS + 1)); else
    FAIL=$((FAIL + 1)); echo "  FAIL param-gap n=$n : exit $got (a conditionally-discharged contract trapped though requires held!)"; fi
done
# the requires-assert is the real guard: a negative b must trap at ENTRY, not silently pass the postcondition
cat > "$T/pgapneg.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine within_b(a: i32, b: i32) -> i32 requires b >= 0 ensures result <= a + b { return a; }
machine Main::main(&mut self) { let w: i32 = within_b(7, 0 - 1); self.console.exit_process(42) }
EOF
EPS_ARCH=aarch64 ./target/debug/beta "$T/pgapneg.alp" "$T/pgapneg" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling parameter-gap negative driver"; exit 1; }
set +e; "$T/pgapneg"; got=$?; set -e
if [ "$got" -gt 128 ]; then PASS=$((PASS + 1)); echo "  ok   param-gap negative b traps at the requires assert (exit $got)";
else FAIL=$((FAIL + 1)); echo "  FAIL param-gap negative b did NOT trap (exit $got) -- the entry guard is missing"; fi

echo "discharge soundness (static proof and runtime assert agree on discharged contracts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
