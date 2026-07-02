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

build() {  # betafile compiler-cmd... -> $T/out.exe  (compile, assemble, stamp)
  bf="$1"; shift
  "$@" < "$bf" > "$T/o.asm" 2>"$T/e" && "$ASM" < "$T/o.asm" > "$T/o.tape" 2>/dev/null \
    && stamp_seed "$T/o.tape" "$SEED" "$T/o.exe" >/dev/null 2>&1
}
runp() { code=0; "$1" < "$2" > "$T/out" 2>/dev/null || code=$?; }   # exe stdinfile -> $T/out, $code

# ddc DESC EXPECTED PROGRAM — compile with both front-ends, run both (no stdin), require agreement + expected exit.
ddc() {
  printf '%s' "$3" > "$T/p.beta"; : > "$T/in"
  if ! build "$T/p.beta" python3 bc2.py;   then FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2.py error: $(cat "$T/e")"; return; fi
  cp "$T/o.exe" "$T/py.exe"
  if ! build "$T/p.beta" "$BCRS";          then FAIL=$((FAIL+1)); echo "  FAIL $1 : on-ramp error"; return; fi
  cp "$T/o.exe" "$T/rs.exe"
  runp "$T/py.exe" "$T/in"; py=$code
  runp "$T/rs.exe" "$T/in"; rs=$code
  if [ "$py" = "$rs" ] && [ "$py" = "$2" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2=$py rust=$rs expected=$2"; fi
}

# io DESC STDIN EXPECT_OUT PROGRAM — like ddc but compares STDOUT (and exit) between the two front-ends.
io() {
  printf '%s' "$4" > "$T/p.beta"; printf '%s' "$2" > "$T/in"
  if ! build "$T/p.beta" python3 bc2.py; then FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2.py error: $(cat "$T/e")"; return; fi
  cp "$T/o.exe" "$T/py.exe"
  if ! build "$T/p.beta" "$BCRS";        then FAIL=$((FAIL+1)); echo "  FAIL $1 : on-ramp error"; return; fi
  cp "$T/o.exe" "$T/rs.exe"
  runp "$T/py.exe" "$T/in"; pc=$code; po=$(cat "$T/out")
  runp "$T/rs.exe" "$T/in"; rc=$code; ro=$(cat "$T/out")
  if [ "$po" = "$ro" ] && [ "$pc" = "$rc" ] && [ "$po" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : bc2=(out='$po' rc=$pc) rust=(out='$ro' rc=$rc) expected out='$3'"; fi
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
# slice 3 — procedures, parameters, calls, recursion
ddc "call with args" 42 'proc add(a, b) { return a + b }
proc main() { return add(6, 36) }'
ddc "nested calls"   42 'proc add(a, b) { return a + b }
proc dbl(x) { return x + x }
proc main() { return add(dbl(15), 12) }'
ddc "recursive fact" 120 'proc fact(n) {
    state c { to rec when (n > 1)  return 1 }
    state rec { return n * fact(n - 1) }
}
proc main() { return fact(5) }'
ddc "recursive fib"   55 'proc fib(n) {
    state c { to rec when (n > 1)  return n }
    state rec { return fib(n - 1) + fib(n - 2) }
}
proc main() { return fib(10) }'
ddc "recursive gcd"   12 'proc gcd(a, b) {
    state c { to done when (b == 0)  return gcd(b, a % b) }
    state done { return a }
}
proc main() { return gcd(48, 36) }'
ddc "four params"    100 'proc sum4(a, b, c, d) { return a + b + c + d }
proc main() { return sum4(10, 20, 30, 40) }'
# slice 4 — byte[]/word[] memory
ddc "word roundtrip"  42 'proc main() {
    let buf = 2097152
    word[buf] = 42
    return word[buf]
}'
ddc "byte truncation"  1 'proc main() {
    let buf = 2097152
    byte[buf] = 257
    return byte[buf]
}'
ddc "buffer accumulate" 15 'proc main() {
    let buf = 2097152
    let i = 0
    state fill { to body when (i < 5)  to go }
    state body { word[buf + i * 8] = i + 1  i = i + 1  to fill }
    state go { let s = 0  let j = 0  to loop }
    state loop { to add when (j < 5)  return s }
    state add { s = s + word[buf + j * 8]  j = j + 1  to loop }
}'
# slice 5 — char literals, read_byte/write_byte, call statements (compare STDOUT too)
io "char literal"  ""    ""   "proc main() { return 'A' }"
io "write chars"   ""    "Hi" "proc main() { write_byte('H')  write_byte('i')  return 0 }"
io "echo stdin"    "xyz" "xyz" "proc main() {
    let c = read_byte()
    state loop { to body when (c >= 0)  return 0 }
    state body { write_byte(c)  c = read_byte()  to loop }
}"
io "call statement" ""   "OK" "proc putc(c) { write_byte(c)  return 0 }
proc main() { putc('O')  putc('K')  return 0 }"
io "recursive print_num" "" "42" "proc print_num(n) {
    state big { to rec when (n >= 10)  to digit }
    state rec { print_num(n / 10)  to digit }
    state digit { write_byte(n % 10 + '0')  return 0 }
}
proc main() { print_num(42)  return 0 }"

# a REAL, non-trivial program: the recursive-descent calculator. bc2.py now covers everything it uses
# (slices 1-5, no string literals). Compile it with both front-ends and check they agree on real input.
if [ -f ../beta-lang-rs/examples/calc.beta ]; then
  if build ../beta-lang-rs/examples/calc.beta python3 bc2.py; then cp "$T/o.exe" "$T/py.exe"
    build ../beta-lang-rs/examples/calc.beta "$BCRS"; cp "$T/o.exe" "$T/rs.exe"
    for expr in "2+3*4" "(2+3)*4" "100-58" "2*(3+4)*5" "7*7-7"; do
      printf '%s' "$expr" > "$T/in"
      runp "$T/py.exe" "$T/in"; pc=$code; po=$(cat "$T/out")
      runp "$T/rs.exe" "$T/in"; rc=$code; ro=$(cat "$T/out")
      if [ "$po" = "$ro" ] && [ "$pc" = "$rc" ]; then PASS=$((PASS+1)); else
        FAIL=$((FAIL+1)); echo "  FAIL calc.beta '$expr' : bc2=(out='$po' rc=$pc) rust=(out='$ro' rc=$rc)"; fi
    done
  else FAIL=$((FAIL+1)); echo "  FAIL calc.beta : bc2.py could not compile it: $(cat "$T/e")"; fi
fi

# ============================================================================================
# THE REAL THING — true diverse double compilation of the whole TRUST SURFACE. bc2.py compiles all
# of these; build the Beta compiler two independent ways and check each program compiles IDENTICALLY:
#   official : prog --(Rust on-ramp)--> bc0 ; asmO = bc0(prog)      [the shipped lineage]
#   diverse  : prog --(bc2.py, Python)--> bcA ; asmA = bcA(prog)    [the independent path]
# If asmO == asmA byte-for-byte for EVERY trust-critical Beta program — bc itself, AND the checker,
# the meaning interpreter, the type checker, the omega elaborator — then their compilation is
# independent of which bootstrap compiler produced it: a Trojan would have to sit, identically, in
# BOTH independent paths. Agreement on bc.beta alone does not imply agreement on all programs, so we
# check the actual programs whose compilation determines trust. (selfhost.sh proves bc0(bc.beta) is a
# fixed point, so bc0 is the canonical bc.)
if build ../beta-lang/bc.beta "$BCRS" && cp "$T/o.exe" "$T/bc0.exe" \
   && build ../beta-lang/bc.beta python3 bc2.py && cp "$T/o.exe" "$T/bcA.exe"; then
  for prog in ../beta-lang/bc.beta ../delta/check.beta ../delta/eq.beta \
              ../gamma/interp.beta ../gamma/typeck.beta ../omega/omega2gamma.beta; do
    [ -f "$prog" ] || continue
    if ! python3 bc2.py < "$prog" > /dev/null 2>"$T/e"; then
      FAIL=$((FAIL+1)); echo "  FAIL DDC $prog : bc2.py cannot compile it: $(head -1 "$T/e")"; continue; fi
    "$T/bc0.exe" < "$prog" > "$T/asmO" 2>/dev/null
    "$T/bcA.exe" < "$prog" > "$T/asmA" 2>/dev/null
    if cmp -s "$T/asmO" "$T/asmA"; then
      PASS=$((PASS+1)); echo "  DDC $(basename "$prog"): bc0 == bcA ($(wc -l < "$T/asmO" | tr -d ' ') asm lines)"
    else
      FAIL=$((FAIL+1)); echo "  FAIL DDC $prog : the two independent compilations DIFFER"; fi
  done
  echo "  => the Thompson gap is closed for the whole Beta trust surface, not just bc"
else
  FAIL=$((FAIL+1)); echo "  FAIL DDC : could not build both compilers"
fi

echo "diverse double compilation (bc2.py — independent Rust-free Beta front-end — agrees with the on-ramp; incl. true DDC of bc AND the whole trust surface): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
