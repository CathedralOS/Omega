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
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "bootstrap paths: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
SAMPLES="$OMEGA_PATH_DELTA/samples"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "discharge-soundness SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "discharge-soundness SKIP — no $t"; exit 0; }; done

cargo build -q 2>/dev/null || { echo "discharge-soundness FAIL — cargo build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
PASS=0; FAIL=0

# 1. the discharged machines must NEVER trap, on any input -> the driver always exits 42.
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/contract-driver.alp" "$T/drv" >/dev/null 2>"$T/e" \
  || { echo "discharge-soundness FAIL — compiling contract-driver:"; sed 's/^/    /' "$T/e"; exit 1; }
for n in 0 1 2 7 42 63 100 127 128 200 254 255; do
  set +e; printf "\\$(printf '%03o' "$n")" | "$T/drv"; got=$?; set -e
  if [ "$got" = 42 ]; then PASS=$((PASS + 1)); else
    FAIL=$((FAIL + 1)); echo "  FAIL n=$n : exit $got (a discharged contract trapped at runtime!)"; fi
done

# 1b. SIGN regression: delta `i32` is SIGNED, but discharge proofs are over proof-kernel NATURALS.
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
DELTA_ARCH=aarch64 ./target/debug/delta "$T/negin.alp" "$T/negin" >/dev/null 2>&1 \
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
DELTA_ARCH=aarch64 ./target/debug/delta "$T/neg.alp" "$T/neg" >/dev/null 2>&1 \
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
DELTA_ARCH=aarch64 ./target/debug/delta "$T/comp.alp" "$T/comp" >/dev/null 2>&1 \
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
DELTA_ARCH=aarch64 ./target/debug/delta "$T/pgap.alp" "$T/pgap" >/dev/null 2>&1 \
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
DELTA_ARCH=aarch64 ./target/debug/delta "$T/pgapneg.alp" "$T/pgapneg" >/dev/null 2>&1 \
  || { echo "discharge-soundness FAIL — compiling parameter-gap negative driver"; exit 1; }
set +e; "$T/pgapneg"; got=$?; set -e
if [ "$got" -gt 128 ]; then PASS=$((PASS + 1)); echo "  ok   param-gap negative b traps at the requires assert (exit $got)";
else FAIL=$((FAIL + 1)); echo "  FAIL param-gap negative b did NOT trap (exit $got) -- the entry guard is missing"; fi

# 5. RANGE-TYPE soundness: a `i: i32 in lo..hi` parameter desugars to entry asserts (lo <= i, i < hi) that
# ENFORCE the bounded type -- the same facts the value-domain static discharge assumes (`self.arr[i]` array
# bounds and `self.arr[i+j]` interval propagation are proved FROM these param types). So the discharge is sound
# only if the type is really enforced: in-range args must never trap, and an out-of-range arg (EITHER bound) must
# trap at the entry assert, not silently pass -- which would make the type, and every access proved from it, unsound.
cat > "$T/rng.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; }
data Main { console: Console; }
machine rcheck(i: i32 in 0..16, j: i32 in 3..8) -> i32 { return i + j; }
machine Main::main(&mut self) {
    let n: i32 = read_byte();       // 0..255
    let i: i32 = n % 16;            // 0..15 -> in 0..16
    let j: i32 = (n % 5) + 3;       // 3..7  -> in 3..8
    let r: i32 = rcheck(i, j);      // both in range -> no entry-assert trap
    self.console.exit_process(42)
}
EOF
DELTA_ARCH=aarch64 ./target/debug/delta "$T/rng.alp" "$T/rng" >/dev/null 2>"$T/e" \
  || { echo "discharge-soundness FAIL — compiling range-type driver"; sed 's/^/    /' "$T/e"; exit 1; }
for n in 0 1 15 16 42 100 200 255; do
  set +e; printf "\\$(printf '%03o' "$n")" | "$T/rng"; got=$?; set -e
  if [ "$got" = 42 ]; then PASS=$((PASS + 1)); else
    FAIL=$((FAIL + 1)); echo "  FAIL range-type n=$n : exit $got (an in-range arg trapped a range-type entry assert!)"; fi
done
# out-of-range must TRAP -- upper (i=16 !< 16) and lower (j=2 !>= 3), separately (a trap ends the process), so
# both directions of the bounded type are enforced, not just one.
for pair in '16, 5|upper i<16' '5, 2|lower 3<=j'; do
  args=${pair%%|*}; desc=${pair##*|}
  cat > "$T/rngbad.alp" <<EOF
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine rcheck(i: i32 in 0..16, j: i32 in 3..8) -> i32 { return i + j; }
machine Main::main(&mut self) { let r: i32 = rcheck($args); self.console.exit_process(42) }
EOF
  DELTA_ARCH=aarch64 ./target/debug/delta "$T/rngbad.alp" "$T/rngbad" >/dev/null 2>&1 \
    || { echo "discharge-soundness FAIL — compiling range-type negative ($desc)"; exit 1; }
  set +e; "$T/rngbad"; got=$?; set -e
  if [ "$got" -gt 128 ]; then PASS=$((PASS + 1)); echo "  ok   range-type out-of-range traps ($desc, exit $got)";
  else FAIL=$((FAIL + 1)); echo "  FAIL range-type out-of-range did NOT trap ($desc, exit $got) -- the type is not enforced"; fi
done

echo "discharge soundness (static proof and runtime assert agree on discharged contracts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
