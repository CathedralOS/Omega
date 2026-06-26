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
command -v cargo     >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no cargo"; exit 0; }
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
# out NAME SOURCE EXPECTED_STDOUT  — host output programs
out_test() {
  build "$1" "$2" || return
  set +e; got=$("$T/out" 2>/dev/null); set -e
  if [ "$got" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : stdout [$got], expected [$3]"; fi
}
# filter NAME SOURCE STDIN EXPECTED_STDOUT  — byte-I/O filters
filter_test() {
  build "$1" "$2" || return
  set +e; got=$(printf '%s' "$3" | "$T/out" 2>/dev/null); set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : in [$3] -> out [$got], expected [$4]"; fi
}
# tokens NAME SOURCE INPUT EXPECTED  — a lexer emitting one token per line;
# compare the token stream space-joined.
tokens_test() {
  build "$1" "$2" || return
  set +e; got=$(printf '%s' "$3" | "$T/out" 2>/dev/null | tr '\n' ' ' | sed 's/ *$//'); set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : [$3] -> [$got], expected [$4]"; fi
}
# compiler NAME SOURCE.alp EXPR EXPECTED_EXIT  — an epsilon-written COMPILER:
# build it, run it on EXPR to emit assembly, assemble+sign+run, check the exit code.
compiler_test() {
  build "$1" "$2" || return                                   # $T/out = the epsilon compiler
  printf '%s' "$3" | "$T/out" > "$T/gen.s" 2>/dev/null
  if clang -arch arm64 -o "$T/gen" "$T/gen.s" 2>"$T/cerr" && codesign -f -s - "$T/gen" 2>/dev/null; then
    set +e; "$T/gen"; got=$?; set -e
    if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL $1 : compiled [$3] -> exit $got, expected $4"; fi
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : emitted asm did not assemble:"; sed 's/^/    /' "$T/cerr"; fi
}
# compiler_trap NAME SOURCE.alp EXPR  — the COMPILED program must trap (>128), i.e.
# the emitted safety check (overflow / div-by-zero) faults at runtime.
compiler_trap() {
  build "$1" "$2" || return
  printf '%s' "$3" | "$T/out" > "$T/gen.s" 2>/dev/null
  if clang -arch arm64 -o "$T/gen" "$T/gen.s" 2>"$T/cerr" && codesign -f -s - "$T/gen" 2>/dev/null; then
    set +e; "$T/gen"; got=$?; set -e
    if [ "$got" -gt 128 ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL $1 : compiled [$3] -> exit $got, expected a trap (>128)"; fi
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : emitted asm did not assemble"; fi
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
# Recursion: a machine calling itself (fact(5) = 120) — recursive descent works.
run "recursion (fact(5))" samples/recursion.alp 120
# Slice 7a: data structs + mutable self fields — sum 1..=5 into self.total -> 15.
run "data (self fields, sum 1..5)" samples/data.alp 15
# Slice 7b: array fields + self/method calls + bounds-checked indexing.
run "methods (stack in self, 7*10+5)" samples/methods.alp 75
cat > "$T/ib.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; arr: [i32; 4]; }
machine Main::main(&mut self) { self.arr[2] = 42; self.console.exit_process(self.arr[2]) }
EOF
run "array write/read in bounds" "$T/ib.alp" 42
cat > "$T/oob.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; arr: [i32; 4]; }
machine Main::main(&mut self) { self.arr[10] = 5; self.console.exit_process(0) }
EOF
trap_test "out-of-bounds index traps" "$T/oob.alp"
# Slice 3 (partial): host output via libSystem write — write_line to stdout.
out_test "hello (write_line to stdout)" samples/hello.alp "hello, alpha"
# Slice 3 complete: byte I/O — read_byte/write_byte filters over real stdin/stdout.
filter_test "echo (read->write byte loop)" samples/echo.alp "hello, bytes!" "hello, bytes!"
filter_test "buffer ([u8;N], reverse stdin)" samples/buffer.alp "abcde" "edcba"
# Number I/O: parse decimal, compute, format decimal back (self-hosting primitive).
filter_test "square (parse+compute+format)" samples/square.alp "144" "20736"
# Recursive-descent calculator — precedence + parens (recursion). Compiler-shaped.
filter_test "calc precedence (2+3*4)"     samples/calc.alp "2+3*4"     "14"
filter_test "calc parens/recursion ((2+3)*4)" samples/calc.alp "(2+3)*4" "20"
filter_test "calc nested parens (((1+2))*3)"  samples/calc.alp "((1+2))*3" "9"
# An expression COMPILER written in epsilon: emits ARM64 asm, assembled+run here.
compiler_test "exprc compiles 2+3*4"   samples/exprc.alp "2+3*4"     14
compiler_test "exprc compiles (2+3)*4" samples/exprc.alp "(2+3)*4"   20
compiler_test "exprc compiles ((1+2))*3" samples/exprc.alp "((1+2))*3" 9
# Full i32 literals: > 65535 needs movz+movk (lo/hi split via division).
compiler_test "exprc large literal (100000/1000)" samples/exprc.alp "100000/1000" 100
# minic: an imperative-language compiler in epsilon — variables + assignment.
compiler_test "minic vars (a=2+3;a*4)"       samples/minic.alp "a=2+3;a*4"          20
compiler_test "minic chained (a=3;b=a+1;c=b*2;c+a)" samples/minic.alp "a=3;b=a+1;c=b*2;c+a" 11
compiler_test "minic var-expr (a=2;b=a*a;b+1)" samples/minic.alp "a=2;b=a*a;b+1"     5
# tokenize: a real .alp lexer in epsilon — the first stage of a self-hosting compiler.
tokens_test "tokenize .alp source" samples/tokenize.alp "machine f(x){return x+1;}" "machine f ( x ) { return x + 1 ; }"
tokens_test "tokenize drops // comments" samples/tokenize.alp "a // c
b/c" "a b / c"
# The emitted code is overflow-SAFE: a compiled overflowing expr traps at runtime.
compiler_trap "exprc emits overflow trap" samples/exprc.alp "46341*46341"
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

echo "aarch64 macOS backend gate (slices 1-7, full parity): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
