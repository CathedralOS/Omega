#!/usr/bin/env sh
# DIVERSE DOUBLE COMPILATION — the Thompson-resistance gate for the Beta rung (decision D5).
#
# bc.beta's only source->assembly path is the Rust on-ramp (beta-lang-rs). Self-host reproduces bc but
# does NOT diversify it: a Trojan the on-ramp injected would ride through the self-host fixed point
# undetected. Wheeler's defence is a SECOND, INDEPENDENT compiler for the same language — here
# ../beta-lang-py/bc2.py, written from scratch in Python against the ISA + the Beta grammar.
#
# This gate compiles a corpus with BOTH front-ends, assembles + runs each output, and asserts the two
# independent compilers AGREE (and match the expected result). bc2.py is UNTRUSTED — a bug or Trojan in
# it makes a disagreement, i.e. a LOUD failure, never a silent pass. Today bc2.py covers slice 1
# (arithmetic + let); as it grows to the whole language the corpus grows toward bc.beta itself, at which
# point this becomes true DDC: bc.beta compiled via the independent path must reproduce the official
# self-host fixed point.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "diverse-double-compilation SKIP — no python3"; exit 0; }
command -v cargo   >/dev/null 2>&1 || { echo "diverse-double-compilation SKIP — no cargo (on-ramp)"; exit 0; }

. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
BCRS=../beta-lang-rs/target/debug/beta-lang
( cd ../beta-lang-rs && cargo build -q 2>/dev/null ) || { echo "diverse-double-compilation FAIL — on-ramp build"; exit 1; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
PASS=0; FAIL=0

run_asm() {  # asmfile -> program exit code (|| guards set -e against the program's own nonzero exit)
  "$ASM" < "$1" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$T/x.exe" >/dev/null 2>&1 || return 1
  code=0; "$T/x.exe" >/dev/null 2>&1 || code=$?; echo "$code"
}

# ddc DESC EXPECTED PROGRAM — compile with both front-ends, run both, require agreement + expected.
ddc() {
  printf '%s' "$3" > "$T/p.beta"
  if ! python3 bc2.py < "$T/p.beta" > "$T/py.asm" 2>"$T/e"; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2.py error: $(cat "$T/e")"; return; fi
  py=$(run_asm "$T/py.asm")
  "$BCRS" < "$T/p.beta" > "$T/rs.asm" 2>/dev/null || { FAIL=$((FAIL+1)); echo "  FAIL $1 : on-ramp error"; return; }
  rs=$(run_asm "$T/rs.asm")
  if [ "$py" = "$rs" ] && [ "$py" = "$2" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2=$py rust=$rs expected=$2"; fi
}

ddc "product"        42 'proc main() { return 6 * 7 }'
ddc "precedence"     14 'proc main() { return 2 + 3 * 4 }'
ddc "parens"         20 'proc main() { return (2 + 3) * 4 }'
ddc "subtraction"    42 'proc main() { return 100 - 58 }'
ddc "division"        7 'proc main() { return 50 / 7 }'
ddc "modulo"          1 'proc main() { return 50 % 7 }'
ddc "nested parens"  70 'proc main() { return 2 * (3 + 4) * 5 }'
ddc "let chain"      42 'proc main() {
    let a = 6 * 7
    let b = a + 8
    return b - 8
}'
ddc "reassign"       30 'proc main() {
    let x = 10
    x = x + 20
    return x
}'
ddc "many locals"    55 'proc main() {
    let a = 1
    let b = a + 4
    let c = b + 9
    let d = c + 16
    let e = d + 25
    return e
}'
# slice 2 — comparisons (0/1) + state/to-when CFG control flow
ddc "less-than"       1 'proc main() { return 5 < 8 }'
ddc "ge boundary"     1 'proc main() { return 5 >= 5 }'
ddc "not-equal"       1 'proc main() { return 7 != 4 }'
ddc "cmp under arith" 42 'proc main() {
    let a = 8
    return (a > 5) * 30 + (a < 5) * 7 + 12
}'
ddc "loop sum 1..4"  10 'proc main() {
    let i = 0
    let s = 0
    state loop { to body when (i < 4)  return s }
    state body { i = i + 1  s = s + i  to loop }
}'
ddc "factorial 5!"  120 'proc main() {
    let i = 1
    let a = 1
    state loop { to body when (i <= 5)  return a }
    state body { a = a * i  i = i + 1  to loop }
}'
ddc "countdown mul"  16 'proc main() {
    let n = 4
    let a = 1
    state loop { to done when (n == 0)  to body }
    state body { a = a * 2  n = n - 1  to loop }
    state done { return a }
}'

echo "diverse double compilation (bc2.py — independent Rust-free Beta front-end — agrees with the on-ramp, both assembled + run): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
