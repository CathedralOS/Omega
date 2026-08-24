#!/usr/bin/env sh
# aarch64/macOS backend gate — the FIRST executable test of delta-rust on this
# platform. The x64 PE backend's output cannot run here; this backend emits ARM64
# assembly, clang assembles+links a Mach-O, codesign ad-hoc signs it, and we run
# the result and check its exit status. That closes the loop: delta is now a
# verifiable rung on macOS arm64, not just a compile-only one.
#
# Requires: clang + codesign (Xcode command line tools) and an arm64 host.
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

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "aarch64 gate SKIP — not macOS arm64"; exit 0 ;;
esac
command -v cargo     >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no cargo"; exit 0; }
command -v clang     >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no clang"; exit 0; }
command -v codesign  >/dev/null 2>&1 || { echo "aarch64 gate SKIP — no codesign"; exit 0; }

cargo build -q 2>/dev/null || { echo "aarch64 gate FAIL — cargo build"; exit 1; }
BIN=./target/debug/delta
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

PASS=0; FAIL=0
# compile NAME SOURCE.alp -> $T/out (binary), or record a failure
build() {
  DELTA_ARCH=aarch64 "$BIN" "$2" "$T/out" >/dev/null 2>"$T/err" || {
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
  set +e
  # lowermachine emits asm (its output IS a program); extract the machine-body window
  # (.align 2 .. Ltrap) so its header + Ltrap + data-section wrapper is ignored by body-parity
  # gates. Other samples are runtime programs -- compare their actual output verbatim.
  if [ "$2" = "$SAMPLES/lowermachine.alp" ]; then
    got=$(printf '%s' "$3" | "$T/out" 2>/dev/null | awk '/^\.align 2/{if(!seen){p=1;seen=1;next}} /^Ltrap:/{p=0} p')
  else
    got=$(printf '%s' "$3" | "$T/out" 2>/dev/null)
  fi
  set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : in [$3] -> out [$got], expected [$4]"; fi
}
# wrap_test NAME SOURCE INPUT EXPECTED — compare the FULL output (header + bodies + Ltrap + data
# sections), for the runnable-object wrapper.
wrap_test() {
  build "$1" "$2" || return
  set +e; got=$(printf '%s' "$3" | "$T/out" 2>/dev/null); set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : in [$3] -> out [$got], expected [$4]"; fi
}
# stdin_exit NAME SOURCE INPUT EXPECTED_EXIT  — feed stdin, check the exit code
stdin_exit() {
  build "$1" "$2" || return
  set +e; printf '%s' "$3" | "$T/out"; got=$?; set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : [$3] -> exit $got, expected $4"; fi
}
# tokens NAME SOURCE INPUT EXPECTED  — a lexer emitting one token per line;
# compare the token stream space-joined.
tokens_test() {
  build "$1" "$2" || return
  set +e; got=$(printf '%s' "$3" | "$T/out" 2>/dev/null | tr '\n' ' ' | sed 's/ *$//'); set -e
  if [ "$got" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : [$3] -> [$got], expected [$4]"; fi
}
# bundle_result INPUT_FILE EXPECTED_EXIT — compile the D0 bundle decoder once,
# then feed one binary fixture without passing NUL bytes through shell variables.
bundle_result() {
  set +e; "$T/out" < "$1" > /dev/null 2>&1; got=$?; set -e
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL Omega0 bundle $3 : exit $got, expected $2"; fi
}
# compiler NAME SOURCE.alp EXPR EXPECTED_EXIT  — a Delta-written COMPILER:
# build it, run it on EXPR to emit assembly, assemble+sign+run, check the exit code.
compiler_test() {
  build "$1" "$2" || return                                   # $T/out = the Delta compiler
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
# selfhost NAME SAMPLE.alp INPUT...  — SECOND-ORDER self-hosting: the self-hosted compiler
# ($T/lmx, the byte-identical lowermachine, built by the fixpoint block) compiles ANOTHER real
# Delta program, and the resulting binary's runtime output must equal the trusted Rust-beta
# reference compilation of the SAME program on every INPUT. Proves lowermachine compiles real
# compilers correctly end-to-end, not just its own source.
selfhost_test() {
  shn="$1"; shs="$2"; shift 2
  DELTA_ARCH=aarch64 "$BIN" "$shs" "$T/shref" >/dev/null 2>"$T/err" && codesign -f -s - "$T/shref" 2>/dev/null || {
    FAIL=$((FAIL+1)); echo "  FAIL $shn : reference build"; return; }
  "$T/lmx" < "$shs" > "$T/shg.s" 2>/dev/null
  clang -arch arm64 -o "$T/shg" "$T/shg.s" 2>"$T/cerr" && codesign -f -s - "$T/shg" 2>/dev/null || {
    FAIL=$((FAIL+1)); echo "  FAIL $shn : lowermachine-emitted asm did not assemble:"; sed 's/^/    /' "$T/cerr"; return; }
  for inp in "$@"; do
    set +e
    a=$(printf '%s' "$inp" | "$T/shref" 2>/dev/null); ax=$?
    b=$(printf '%s' "$inp" | "$T/shg" 2>/dev/null); bx=$?
    set -e
    if [ "$a" = "$b" ] && [ "$ax" = "$bx" ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL $shn : in [$inp] -> lm [$b](exit $bx), ref [$a](exit $ax)"; fi
  done
}
# selfhost_file_test NAME SOURCE INPUT_FILE — binary-input counterpart to
# selfhost_test, used where the fixture contains NUL framing bytes.
selfhost_file_test() {
  shn=$1; shs=$2; input_file=$3
  DELTA_ARCH=aarch64 "$BIN" "$shs" "$T/shref" >/dev/null 2>"$T/err" && codesign -f -s - "$T/shref" 2>/dev/null || {
    FAIL=$((FAIL+1)); echo "  FAIL $shn : reference build"; return; }
  "$T/lmx" < "$shs" > "$T/shg.s" 2>/dev/null
  clang -arch arm64 -o "$T/shg" "$T/shg.s" 2>"$T/cerr" && codesign -f -s - "$T/shg" 2>/dev/null || {
    FAIL=$((FAIL+1)); echo "  FAIL $shn : lowermachine-emitted asm did not assemble:"; sed 's/^/    /' "$T/cerr"; return; }
  set +e
  "$T/shref" < "$input_file" > "$T/ref.out" 2>/dev/null; ref_exit=$?
  "$T/shg" < "$input_file" > "$T/self.out" 2>/dev/null; self_exit=$?
  set -e
  if [ "$ref_exit" = "$self_exit" ] && cmp -s "$T/ref.out" "$T/self.out"; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $shn : self=$self_exit reference=$ref_exit"
  fi
}

# Slice 1: exit_process(<const>) -> the constant is the process exit status.
run "exit7 (exit_process(7))" "$SAMPLES/exit7.alp" 7
# Slice 2: expressions + locals + arithmetic precedence.
run "arith (3 + 4*2)"          "$SAMPLES/arith.alp"  11
run "locals (a=10; b=a-3; b*2)" "$SAMPLES/locals.alp" 14
run "modulo (% operator + precedence)" "$SAMPLES/modulo.alp" 222
run "bitwise (& | ^ + precedence)" "$SAMPLES/bitops.alp" 125
run "shifts (<< >> + field-build idiom)" "$SAMPLES/shifts.alp" 129
run "min/max (clamp idiom max(0,min(v,60)))" "$SAMPLES/minmax.alp" 200
run "unary minus (-x, negative literals)" "$SAMPLES/negate.alp" 36
run "tag-only enum (decl + ::variant + exhaustive match)" "$SAMPLES/enum.alp" 7
run "state params (loop carries i,acc -> 16)" "$SAMPLES/stateparams.alp" 16
run "assert (runtime contract, passing)" "$SAMPLES/assert.alp" 42
run "requires (precondition contract, satisfied)" "$SAMPLES/requires.alp" 42
run "ensures (postcondition contract, satisfied)" "$SAMPLES/ensures.alp" 42
run "enum payload (construct + match-bind + extract)" "$SAMPLES/payload.alp" 42
run "multi-field payload (Rectangle{w,h} -> w*h)" "$SAMPLES/shape.alp" 42
# Slices 4-5: transition/state control flow + back-edge loop + reassignment.
# (write_line is a no-op here; the exit code is fixed by the control flow: i 0->3.)
run "loop (count i 0->3, exit i)" "$SAMPLES/loop.alp" 3
# Slice 6: machine calls (the DAG) — max(7,19)=19, max(19,12)=19, callee has states.
run "calls (max(max(7,19),12))" "$SAMPLES/calls.alp" 19
# Recursion: a machine calling itself (fact(5) = 120) — recursive descent works.
run "recursion (fact(5))" "$SAMPLES/recursion.alp" 120
# Slice 7a: data structs + mutable self fields — sum 1..=5 into self.total -> 15.
run "data (self fields, sum 1..5)" "$SAMPLES/data.alp" 15
# Slice 7b: array fields + self/method calls + bounds-checked indexing.
run "methods (stack in self, 7*10+5)" "$SAMPLES/methods.alp" 75
run "bootstrap storage (aligned bump allocation + reset)" "$SAMPLES/bootstrap-storage.alp" 42
if build "omega-bootstrap canonical bundle decoder" "$SAMPLES/omega-bootstrap-bundle-decode.alp"; then
  # One canonical entry: label `main.omg`, content `abc`. Its byte checksum mod
  # 251 is 80. The other fixtures exercise framing, paths, ordering, and the
  # decoder's explicit local resource ceiling.
  printf 'OMG0BNDL\001\000\000\000\001\000\000\000\010\000\000\000\003\000\000\000main.omgabc' > "$T/bundle-ok"
  cp "$T/bundle-ok" "$T/bundle-trailing"; printf x >> "$T/bundle-trailing"
  printf 'OMG0BNDL\001\000\000\000\001\000\000\000\010\000\000\000\003\000\000\000main.omgab' > "$T/bundle-short"
  printf 'OMG0BNDL\001\000\000\000\001\000\000\000\101\000\000\000\000\000\000\000' > "$T/bundle-exhausted"
  printf 'OMG0BNDL\001\000\000\000\002\000\000\000\005\000\000\000\000\000\000\000z.omg\005\000\000\000\000\000\000\000a.omg' > "$T/bundle-order"
  printf 'OMG0BNDL\001\000\000\000\001\000\000\000\004\000\000\000\000\000\000\000../x' > "$T/bundle-path"
  bundle_result "$T/bundle-ok" 80 "canonical input"
  bundle_result "$T/bundle-trailing" 251 "trailing byte"
  bundle_result "$T/bundle-short" 251 "truncated content"
  bundle_result "$T/bundle-exhausted" 252 "checked label-storage exhaustion"
  bundle_result "$T/bundle-order" 251 "noncanonical label order"
  bundle_result "$T/bundle-path" 251 "unsafe label"
fi
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
out_test "hello (write_line to stdout)" "$SAMPLES/hello.alp" "hello, alpha"

# DOMAIN TYPES — `i32 in Wrapping` (Omega's arithmetic-safety model). Same overflowing add (2e9 + 2e9):
# the Wrapping `let` omits the overflow trap and wraps to a negative value (reaches exit 42); without the
# domain annotation, the default trapping arithmetic dies on the overflow (a trap, exit > 128).
stdin_exit "i32 in Wrapping: overflowing add wraps, no trap" "$SAMPLES/wraptest.alp" "" 42
sed 's/ in Wrapping//' "$SAMPLES/wraptest.alp" > "$T/wraptrap.alp"
trap_test "default i32 overflow traps (no Wrapping domain)" "$T/wraptrap.alp"
# `i32 in Saturating` clamps the overflowing add to i32 MAX (2147483647) -> reaches exit 42.
stdin_exit "i32 in Saturating: overflowing add clamps to i32 MAX" "$SAMPLES/sattest.alp" "" 42
# The arithmetic domain on a FIELD's type: `self.v = self.v + ...` clamps because `v: i32 in Saturating`.
stdin_exit "i32 in Saturating FIELD: direct self.field add clamps to MAX" "$SAMPLES/fieldsat.alp" "" 42
sed 's/ in Saturating//' "$SAMPLES/fieldsat.alp" > "$T/fieldtrap.alp"
trap_test "plain i32 field overflow traps (no domain on the field)" "$T/fieldtrap.alp"
# Nested data field access (self.a.b): the real Omega bounded_counter shape — read+write a nested struct's
# scalar field via flat-offset resolution over the inlined layout.
stdin_exit "nested data field access (self.counter.value read+write)" "$SAMPLES/nested.alp" "" 42
# Slice 3 complete: byte I/O — read_byte/write_byte filters over real stdin/stdout.
filter_test "echo (read->write byte loop)" "$SAMPLES/echo.alp" "hello, bytes!" "hello, bytes!"
filter_test "buffer ([u8;N], reverse stdin)" "$SAMPLES/buffer.alp" "abcde" "edcba"
# Number I/O: parse decimal, compute, format decimal back (self-hosting primitive).
filter_test "square (parse+compute+format)" "$SAMPLES/square.alp" "144" "20736"
# Recursive-descent calculator — precedence + parens (recursion). Compiler-shaped.
filter_test "calc precedence (2+3*4)"     "$SAMPLES/calc.alp" "2+3*4"     "14"
filter_test "calc parens/recursion ((2+3)*4)" "$SAMPLES/calc.alp" "(2+3)*4" "20"
filter_test "calc nested parens (((1+2))*3)"  "$SAMPLES/calc.alp" "((1+2))*3" "9"
# An expression COMPILER written in delta: emits ARM64 asm, assembled+run here.
compiler_test "exprc compiles 2+3*4"   "$SAMPLES/exprc.alp" "2+3*4"     14
compiler_test "exprc compiles (2+3)*4" "$SAMPLES/exprc.alp" "(2+3)*4"   20
compiler_test "exprc compiles ((1+2))*3" "$SAMPLES/exprc.alp" "((1+2))*3" 9
# Full i32 literals: > 65535 needs movz+movk (lo/hi split via division).
compiler_test "exprc large literal (100000/1000)" "$SAMPLES/exprc.alp" "100000/1000" 100
# exprc: the operator set caught up to the backend -- % (multiplicative) + bit/shift level.
compiler_test "exprc modulo (7%3)"          "$SAMPLES/exprc.alp" "7%3"       1
compiler_test "exprc bitwise-and (12&10)"   "$SAMPLES/exprc.alp" "12&10"     8
compiler_test "exprc bitwise-or (2|1)"      "$SAMPLES/exprc.alp" "2|1"       3
compiler_test "exprc bitwise-xor (6^3)"     "$SAMPLES/exprc.alp" "6^3"       5
compiler_test "exprc shift-left (1<<4)"     "$SAMPLES/exprc.alp" "1<<4"      16
compiler_test "exprc shift-right (16>>2)"   "$SAMPLES/exprc.alp" "16>>2"     4
compiler_test "exprc bit-op precedence (1+2&3 = 3)" "$SAMPLES/exprc.alp" "1+2&3" 3
# minic: an imperative-language compiler in delta — variables + assignment.
compiler_test "minic vars (a=2+3;a*4)"       "$SAMPLES/minic.alp" "a=2+3;a*4"          20
compiler_test "minic chained (a=3;b=a+1;c=b*2;c+a)" "$SAMPLES/minic.alp" "a=3;b=a+1;c=b*2;c+a" 11
compiler_test "minic var-expr (a=2;b=a*a;b+1)" "$SAMPLES/minic.alp" "a=2;b=a*a;b+1"     5
# minic: operator set caught up to the backend (% and the bit/shift level), over variables.
compiler_test "minic modulo (a=7;a%3)"        "$SAMPLES/minic.alp" "a=7;a%3"       1
compiler_test "minic bitwise-and (a=12;a&10)" "$SAMPLES/minic.alp" "a=12;a&10"     8
compiler_test "minic bitwise-or (a=3;b=5;a|b)" "$SAMPLES/minic.alp" "a=3;b=5;a|b"  7
compiler_test "minic shift-left (a=1;a<<4)"   "$SAMPLES/minic.alp" "a=1;a<<4"      16
compiler_test "minic shift-right (a=16;a>>2)" "$SAMPLES/minic.alp" "a=16;a>>2"     4
# tokenize: a real .alp lexer in delta — the first stage of a self-hosting compiler.
tokens_test "tokenize .alp source" "$SAMPLES/tokenize.alp" "machine f(x){return x+1;}" "machine f ( x ) { return x + 1 ; }"
tokens_test "tokenize drops // comments" "$SAMPLES/tokenize.alp" "a // c
b/c" "a b / c"
tokens_test "tokenize multi-char ops" "$SAMPLES/tokenize.alp" "Main::main n<=3 a->b x==y i-1" "Main :: main n <= 3 a -> b x == y i - 1"
# balance: a bracket-balance validator for .alp (typed stack; skips comments/strings).
stdin_exit "balance ok (nested/mixed + comment/string)" "$SAMPLES/balance.alp" 'f(){ a[0]=1; } // ) ]
x="}"' 0
stdin_exit "balance bad (type mismatch)" "$SAMPLES/balance.alp" 'f( ]' 1
# decls: keyword recognition + structure extraction — list .alp machine names.
tokens_test "decls lists machine names" "$SAMPLES/decls.alp" "machine max(a,b){} machine Main::main(){}" "max Main"
tokens_test "decls keyword vs prefix (machinery != machine)" "$SAMPLES/decls.alp" "machinery x; machine f(){}" "f"
tokens_test "decls lists data + machine names" "$SAMPLES/decls.alp" "data Pt{x:i32;} machine f(){}" "Pt f"
tokens_test "decls keyword vs prefix (database != data)" "$SAMPLES/decls.alp" "database x; data D{} machine M(){}" "D M"
# certify-add emits a proof certificate for a+b (verified end-to-end by
# convergence.sh; here just check the delta side produces the exact certificate).
filter_test "certify-add emits a delta cert" "$SAMPLES/certify-add.alp" "2 3" "(= (p (s (s z)) (s (s (s z)))) (s (s (s (s (s z)))))) (refl (s (s (s (s (s z))))))"
filter_test "certify-lt emits an existential-witness cert" "$SAMPLES/certify-lt.alp" "2 5" "(Exists (= (p (s (s z)) (s (v 0))) (s (s (s (s (s z))))))) (wit (= (p (s (s z)) (s (v 0))) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z)))))))"
filter_test "certify-bounds emits a 2D array-bounds VC" "$SAMPLES/certify-bounds.alp" "2 5 3 4" "(Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z))))))))"
filter_test "certify-divides emits a divisibility cert" "$SAMPLES/certify-divides.alp" "3 12" "(Exists (= (m (v 0) (s (s (s z)))) (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (wit (= (m (v 0) (s (s (s z)))) (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))) (s (s (s (s z)))) (refl (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))))"
filter_test "certify-accesses emits a whole-program safety proof" "$SAMPLES/certify-accesses.alp" "2 5 3 4  1 3 0 2" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))) (s (s z)) (refl (m (s (s z)) (s (s (s z)))))))"
filter_test "certify-safety emits a mixed bounds+nonzero proof" "$SAMPLES/certify-safety.alp" "b 2 5 3 4  d 7  b 1 3 0 2" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (& (Exists (= (p z (s (v 0))) (s (s (s (s (s (s (s z))))))))) (Exists (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z)))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (pair (wit (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))) (s (s (s (s (s (s z)))))) (refl (s (s (s (s (s (s (s z))))))))) (wit (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))) (s (s z)) (refl (m (s (s z)) (s (s (s z))))))))"
filter_test "certify-source compiles source to a safety proof" "$SAMPLES/certify-source.alp" "arr 4 5  get 2 3  div 7" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))) (s (s (s (s (s (s z)))))) (refl (s (s (s (s (s (s (s z))))))))))"
stdin_exit "certify-source rejects unsafe access (exit 1)" "$SAMPLES/certify-source.alp" "arr 4 5  get 3 7" 1
stdin_exit "certify-source accepts safe code (exit 0)" "$SAMPLES/certify-source.alp" "arr 4 5  get 2 3" 0
# statecheck: a real .alp frontend pass (name resolution) written in .alp
stdin_exit "statecheck accepts clean .alp (exit 0)" "$SAMPLES/statecheck.alp" "machine M::m(){ state a(){ transition 0 { _ -> a() } } }" 0
stdin_exit "statecheck rejects an undefined transition target (exit 1)" "$SAMPLES/statecheck.alp" "machine M::m(){ state a(){ transition 0 { _ -> zz() } } }" 1
# dupcheck: duplicate-state detection, machine-SCOPED (a real .alp frontend pass in .alp)
stdin_exit "dupcheck accepts unique states (exit 0)" "$SAMPLES/dupcheck.alp" "machine M::m(){ state a(){} state b(){} }" 0
stdin_exit "dupcheck rejects a duplicate state (exit 1)" "$SAMPLES/dupcheck.alp" "machine M::m(){ state a(){} state a(){} }" 1
stdin_exit "dupcheck is machine-scoped (same name diff machines, exit 0)" "$SAMPLES/dupcheck.alp" "machine A::a(){ state s(){} } machine B::b(){ state s(){} }" 0
# unreachable: dead-state (orphan) detection, machine-scoped -- converse of statecheck
stdin_exit "unreachable accepts all-reached states (exit 0)" "$SAMPLES/unreachable.alp" "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> a() } } }" 0
stdin_exit "unreachable flags an orphan/dead state (exit 1)" "$SAMPLES/unreachable.alp" "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> a() } } state dead(){} }" 1
# fieldcheck: member name resolution -- every self.X field access must be a declared field
stdin_exit "fieldcheck accepts valid fields (exit 0)" "$SAMPLES/fieldcheck.alp" "data Main{ x: i32; } machine A::a(&mut self){ state s(){ self.x = 1; } }" 0
stdin_exit "fieldcheck flags an unknown field (exit 1)" "$SAMPLES/fieldcheck.alp" "data Main{ x: i32; } machine A::a(&mut self){ state s(){ self.y = 1; } }" 1
# methodcheck: method-call name resolution -- every self.m() call names a declared machine method
stdin_exit "methodcheck accepts a declared method call (exit 0)" "$SAMPLES/methodcheck.alp" "machine A::a(&mut self){ state s(){ self.b(); } } machine A::b(&mut self){ state t(){} }" 0
stdin_exit "methodcheck flags an unknown method call (exit 1)" "$SAMPLES/methodcheck.alp" "machine A::a(&mut self){ state s(){ self.b(); } }" 1
# layout: data-struct field-offset computation (BUILDING, not checking) -- reproduces the backend layout
filter_test "layout computes data-struct field offsets (Console=0, i32=8, [u8;N]=N)" "$SAMPLES/layout.alp" "boundary trait C{} data Main{ c: C; n: i32; buf: [u8; 16]; m: i32; }" "$(printf 'c 0\nn 0\nbuf 8\nm 24')"
# cfg: control-flow-graph edge emission (BUILDING) -- from->target edges, enclosing-state tracked
filter_test "cfg emits control-flow graph edges (from target, machine-scoped)" "$SAMPLES/cfg.alp" "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf 'entry a\na b')"
# labels: state -> Lm<mi>s<si> assignment (BUILDING) -- reproduces the backend label scheme EXACTLY
filter_test "labels assigns backend state labels (Lm<mi>s<si>, depth-aware)" "$SAMPLES/labels.alp" "machine A::a(){ state p(){} state q(){} } machine B::b(){ state r(){} }" "$(printf 'p Lm0s0\nq Lm0s1\nr Lm1s0')"
# branches: CODEGEN -- lower transition arms to b/b.eq Lm<mi>s<si> with target resolution
filter_test "branches lowers transition arms to resolved b/b.eq labels" "$SAMPLES/branches.alp" "machine M::m(){ transition 0 { _ -> a() } state a(){ transition self.x { true -> b() false -> a() } } state b(){} }" "$(printf 'b Lm0s0\nb.eq Lm0s1\nb.eq Lm0s0')"
# armdispatch: CODEGEN -- the arm-DISPATCH half of a transition (subject-pop + per-arm cmp/branch)
filter_test "armdispatch lowers a transition's arm dispatch (pop + value-arm cmp/b.eq)" "$SAMPLES/armdispatch.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1')"
# lowertrans: CODEGEN -- a COMPLETE transition (subject lowering fused with arm dispatch)
filter_test "lowertrans lowers a complete transition (self.field subject + pop + arms)" "$SAMPLES/lowertrans.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1')"
# lowerbody: CODEGEN -- a whole MACHINE BODY (per-state labels + every transition lowered)
filter_test "lowerbody emits the machine control skeleton (labels + all transitions)" "$SAMPLES/lowerbody.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:')"
# lowermachine: CODEGEN -- THE ORCHESTRATOR: a complete machine (scaffold wrapped around lowerbody)
filter_test "lowermachine emits a complete machine (_main: prologue + body + epilogue)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine operator subject: a comparison subject (cmp + cset) lowered inside the complete machine
filter_test "lowermachine lowers a comparison subject (self.x < 0 -> cmp/cset)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x < 0 { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, lt\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine bit-ops: prec -1 (below comparisons=0), needed the paren/array sentinel shift -1 -> -3
filter_test "lowermachine lowers a bitwise-and subject (self.x & 3 -> and, prec -1)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x & 3 { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #3\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    and w0, w0, w1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
filter_test "lowermachine lowers a left-shift subject (self.x << 2 -> lsl, two-char)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x << 2 { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    lsl w0, w0, w1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine assignment statements: self.field = EXPR; lowered as RHS + storefield, inside the machine
filter_test "lowermachine lowers assignment statements (self.field = EXPR -> RHS + storefield)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; y: i32; } machine Main::main(&mut self) { self.x = 5; transition 0 { _ -> a() } state a(){ self.y = self.x; transition 0 { _ -> b() } } state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine locals: let declarations size the frame (align16(24+8*lc), self_disp 16+8*lc) + let-store
filter_test "lowermachine sizes the frame for locals and lowers let-store" "$SAMPLES/lowermachine.alp" "data Main { y: i32; } machine Main::main(&mut self) { let x: i32 = 5; self.y = 9; transition 0 { _ -> a() } state a(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #24]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    str w0, [x29, #16]\n    movz w0, #9\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #24]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine local-read: a local used as a value -> ldr w0,[x29,#disp] + push
filter_test "lowermachine reads a local as an operand (let x; self.y = x)" "$SAMPLES/lowermachine.alp" "data Main { y: i32; } machine Main::main(&mut self) { let x: i32 = 5; self.y = x; transition 0 { _ -> a() } state a(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #24]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    str w0, [x29, #16]\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #24]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine multi-machine: emits each machine with its own scaffold (entry _main loads _selfdata; others Lmachine<mi> take self in x0)
filter_test "lowermachine emits multiple machines (entry _main + Lmachine1)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} } machine Main::helper(&mut self) { transition 0 { _ -> h() } state h(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str x0, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine no-arg method call: self.m() -> resolve callee via machine table -> self x0 / bl / push / discard
filter_test "lowermachine lowers a no-arg method call (self.helper() -> bl Lmachine1)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { self.helper(); transition 0 { _ -> a() } state a(){} } machine Main::helper(&mut self) { transition 0 { _ -> h() } state h(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x0, [x29, #16]\n    bl Lmachine1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str x0, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine method call with args: caller pushes/pops args; callee prologue stores param regs + sizes frame
filter_test "lowermachine lowers a method call with an arg (caller push/pop + callee param store)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { self.helper(5); transition 0 { _ -> a() } state a(){} } machine Main::helper(&mut self, n: i32) { transition 0 { _ -> h() } state h(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w1, [x29, #16]\n    str x0, [x29, #24]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine return statement: return EXPR -> lower value + pop w0 + epilogue (no mov w0,#0 default)
filter_test "lowermachine lowers a return statement (return EXPR -> value + epilogue)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) -> i32 { transition 0 { _ -> a() } state a(){ return 7; } }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    movz w0, #7\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine param read: a machine reading its own parameter -> ldr w0,[x29,#(16+8*idx)] (machine-scoped name table)
filter_test "lowermachine reads a parameter (machine-scoped param table)" "$SAMPLES/lowermachine.alp" "data Main { x: i32; } machine Main::main(&mut self) { self.f(5); transition 0 { _ -> a() } state a(){} } machine Main::f(&mut self, n: i32) -> i32 { transition 0 { _ -> a() } state a(){ return n; } }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w1, [x29, #16]\n    str x0, [x29, #24]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine locals-after-params: a local in a param-bearing machine sits at slot 16+8*(pcs+localidx), machine-scoped
filter_test "lowermachine places locals after params (16+8*(pcs+idx), mi-scoped)" "$SAMPLES/lowermachine.alp" "data Main { y: i32; } machine Main::main(&mut self) { self.f(5); transition 0 { _ -> a() } state a(){} } machine Main::f(&mut self, n: i32) -> i32 { let x: i32 = n; transition 0 { _ -> a() } state a(){ return x; } }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #48\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w1, [x29, #16]\n    str x0, [x29, #32]\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    str w0, [x29, #24]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    ldr w0, [x29, #24]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #48\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #48\n    ret')"
# lowermachine field layout: real offsets (console=0, scalar=8, [u8;N]=N, [i32;N]=8N) + boundary-trait skip
filter_test "lowermachine computes real field offsets + skips the boundary trait" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); }
data Main { console: Console; a: i32; buf: [u8; 4]; b: i32; arr: [i32; 3]; c: i32; } machine Main::main(&mut self) { self.a = 1; self.b = 2; self.c = 3; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #12]\n    movz w0, #3\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #44]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# sizeof: align8 total struct size (frame size) -- matches the backend _selfdata directive
filter_test "sizeof emits align8 total struct size" "$SAMPLES/sizeof.alp" "boundary trait C{} data Main{ c: C; n: i32; buf: [u8; 13]; }" "24"
# rpn: infix -> RPN (shunting-yard) -- the expression-lowering arc, step 1 (linearize to stack-machine order)
filter_test "rpn linearizes an expression to postfix (precedence + assoc)" "$SAMPLES/rpn.alp" "e - s == 5" "$(printf 'e\ns\n-\n5\n==')"
filter_test "rpn handles array index (self.buf[i] -> index RPN then base[])" "$SAMPLES/rpn.alp" "self.buf[s] == 115" "$(printf 's\nself.buf[]\n115\n==')"
# rpn: precedence table caught up to the backend (% at level 3; & | ^ << >> at level 0, lowest)
filter_test "rpn modulo (% same prec as *)" "$SAMPLES/rpn.alp" "2 * 3 % 4" "$(printf '2\n3\n*\n4\n%%')"
filter_test "rpn shift-left two-char (<< prec 0, lowest)" "$SAMPLES/rpn.alp" "a << 1 + 2" "$(printf 'a\n1\n2\n+\n<<')"
filter_test "rpn bitwise-and lowest precedence (1+2&3)" "$SAMPLES/rpn.alp" "1 + 2 & 3" "$(printf '1\n2\n+\n3\n&')"
filter_test "rpn shift-right + bitwise-or" "$SAMPLES/rpn.alp" "16 >> 2 | 1" "$(printf '16\n2\n>>\n1\n|')"
# loadk: ARM64 constant materialization (movz/movk) -- first asm-emitting expression primitive
filter_test "loadk emits ARM64 constant load (movz + movk high half)" "$SAMPLES/loadk.alp" "100000" "$(printf 'movz w0, #34464\nmovk w0, #1, lsl #16')"
# lowerop: binary-operator ARM64 snippets (static half of expression lowering)
filter_test "lowerop emits the backend binary-op snippet (<= -> cmp/cset le)" "$SAMPLES/lowerop.alp" "<=" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, le\n    str x0, [sp, #-16]!')"
filter_test "lowerop modulo snippet (% -> sdiv/msub, like aarch64.rs Rem)" "$SAMPLES/lowerop.alp" "%" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cbz w1, Ltrap\n    sdiv w2, w0, w1\n    msub w0, w2, w1, w0\n    str x0, [sp, #-16]!')"
filter_test "lowerop bitwise-and snippet (& -> and)" "$SAMPLES/lowerop.alp" "&" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    and w0, w0, w1\n    str x0, [sp, #-16]!')"
filter_test "lowerop bitwise-or/xor snippet (| -> orr)" "$SAMPLES/lowerop.alp" "|" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    orr w0, w0, w1\n    str x0, [sp, #-16]!')"
filter_test "lowerop shift-left snippet (<< -> lsl, two-char)" "$SAMPLES/lowerop.alp" "<<" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    lsl w0, w0, w1\n    str x0, [sp, #-16]!')"
filter_test "lowerop shift-right snippet (>> -> asr, two-char)" "$SAMPLES/lowerop.alp" ">>" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    asr w0, w0, w1\n    str x0, [sp, #-16]!')"
# fieldload: self.field operand lowering -- the symbol-table half (field name -> layout offset -> SelfField load)
filter_test "fieldload lowers a self.field read to its backend SelfField load" "$SAMPLES/fieldload.alp" "data Main { a: i32; b: i32; c: i32; } machine Main::main(&mut self) { transition self.b { _ -> h() } state h(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!')"
# lowerexpr: the ORCHESTRATOR -- lower a complete expression end-to-end (rpn + loadk + lowerop composed)
filter_test "lowerexpr lowers a full expression (2 + 3) to the backend instruction sequence" "$SAMPLES/lowerexpr.alp" "2 + 3" "$(printf '    movz w0, #2\n    str x0, [sp, #-16]!\n    movz w0, #3\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!')"
filter_test "lowerexpr lowers a bitwise-and (1 & 2 -> and)" "$SAMPLES/lowerexpr.alp" "1 & 2" "$(printf '    movz w0, #1\n    str x0, [sp, #-16]!\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    and w0, w0, w1\n    str x0, [sp, #-16]!')"
filter_test "lowerexpr lowers a two-char shift (8 >> 2 -> asr)" "$SAMPLES/lowerexpr.alp" "8 >> 2" "$(printf '    movz w0, #8\n    str x0, [sp, #-16]!\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    asr w0, w0, w1\n    str x0, [sp, #-16]!')"
# lowersubj: lower a real transition subject (literals + self.field operands + operators)
filter_test "lowersubj lowers a self.field transition subject end-to-end" "$SAMPLES/lowersubj.alp" "data Main { a: i32; b: i32; c: i32; } machine Main::main(&mut self) { transition self.b < 5 { _ -> h() } state h(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, lt\n    str x0, [sp, #-16]!')"
# arrindex: array-index operand (SelfIndex) -- bounds-checked element load, the last operand kind
filter_test "arrindex emits the backend SelfIndex sequence (u8 array, offset 0 count 16)" "$SAMPLES/arrindex.alp" "0 16 1" "$(printf '    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldrb w0, [x9]\n    str x0, [sp, #-16]!')"
# storefield: StoreSelfField (the store half of an assignment) -- statement lowering begins
filter_test "storefield emits the backend field-store (offset 8)" "$SAMPLES/storefield.alp" "8" "$(printf '    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]')"
# storeindex: StoreSelfIndex (array element store) -- self.arr[i] = value
filter_test "storeindex emits the backend array-store (u8 array, offset 0 count 16)" "$SAMPLES/storeindex.alp" "0 16 1" "$(printf '    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldr x1, [sp], #16\n    strb w1, [x9]')"
# scaffold: the machine FRAME (prologue + trailing default + epilogue) for the entry _main
filter_test "scaffold emits the entry-machine frame (local_count 0 -> frame 32)" "$SAMPLES/scaffold.alp" "0" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# selfcall: method call (SelfCall) -- pop args, self in x0, bl callee, push return
filter_test "selfcall emits the backend method-call sequence (1 arg)" "$SAMPLES/selfcall.alp" "1 Lmachine3" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine3\n    str x0, [sp, #-16]!')"
# freecall: free-machine call (Call) -- args x0.., no self, bl callee, push
filter_test "freecall emits the backend free-call sequence (2 args)" "$SAMPLES/freecall.alp" "2 Lmachine5" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    bl Lmachine5\n    str x0, [sp, #-16]!')"
# layout regression: a last field with no trailing semicolon (data ... i32 }) must still parse
filter_test "layout handles last field without trailing semicolon" "$SAMPLES/layout.alp" "boundary trait C{} data Main{ c: C; n: i32 }" "$(printf 'c 0\nn 0')"
stdin_exit "certify-source rejects an overrunning loop (exit 1)" "$SAMPLES/certify-source.alp" "arr 4 5  band 5 0" 1
filter_test "certify-source unrolls a bounded loop (band)" "$SAMPLES/certify-source.alp" "arr 4 5  band 3 0" "(& (Exists (= (p (p (m z (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (& (Exists (= (p (p (m (s z) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))))) (pair (wit (= (p (p (m z (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (pair (wit (= (p (p (m (s z) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s z))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z))))))))))"
filter_test "certify-linked emits a lemma-citing proof" "$SAMPLES/certify-linked.alp" "2 5 3 4" "(Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (app (app (inst (inst (inst (inst (use 30) (s (s z))) (s (s (s (s (s z)))))) (s (s (s z)))) (s (s (s (s z))))) (wit (= (p (s (s z)) (s (v 0))) (s (s (s (s z))))) (s z) (refl (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (s (v 0))) (s (s (s (s (s z)))))) (s z) (refl (s (s (s (s (s z))))))))"
filter_test "certify-loop emits a forall-loop proof citing two lemmas" "$SAMPLES/certify-loop.alp" "4 5 0 4" "(All (-> (Exists (= (p (v 1) (s (v 0))) (s (s (s (s z)))))) (Exists (= (p (p (m (v 1) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))))) (gen (lam (Exists (= (p (v 1) (s (v 0))) (s (s (s (s z)))))) (app (app (inst (inst (inst (inst (use 30) (v 0)) (s (s (s (s (s z)))))) z) (s (s (s (s z))))) (app (app (inst (inst (inst (use 34) (v 0)) (s (s (s (s z))))) (s (s (s (s z))))) (hyp 0)) (wit (= (p (s (s (s (s z)))) (v 0)) (s (s (s (s z))))) z (refl (s (s (s (s z)))))))) (wit (= (p z (s (v 0))) (s (s (s (s (s z)))))) (s (s (s (s z)))) (refl (s (s (s (s (s z))))))))))"
filter_test "certify-mul cites the overflow theorem" "$SAMPLES/certify-mul.alp" "3 4 5 6" "(Exists (= (p (m (s (s (s z))) (s (s (s (s z))))) (s (v 0))) (m (s (s (s (s (s z))))) (s (s (s (s (s (s z))))))))) (app (app (inst (inst (inst (inst (use 66) (s (s (s z)))) (s (s (s (s z))))) (s (s (s (s (s z)))))) (s (s (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (s (v 0))) (s (s (s (s (s z)))))) (s z) (refl (s (s (s (s (s z)))))))) (wit (= (p (s (s (s (s z)))) (s (v 0))) (s (s (s (s (s (s z))))))) (s z) (refl (s (s (s (s (s (s z)))))))))"
filter_test "certify-max proves its result meets the max spec" "$SAMPLES/certify-max.alp" "5 3" "(& (& (Exists (= (p (s (s (s (s (s z))))) (v 0)) (s (s (s (s (s z))))))) (Exists (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))))) (+ (= (s (s (s (s (s z))))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z)))))) (pair (pair (wit (= (p (s (s (s (s (s z))))) (v 0)) (s (s (s (s (s z)))))) z (refl (s (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z)))))))) (inl (= (s (s (s (s (s z))))) (s (s (s z)))) (refl (s (s (s (s (s z))))))))"
filter_test "certify-sort2 proves ordered + permutation" "$SAMPLES/certify-sort2.alp" "5 3" "(& (Exists (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z))))))) (+ (& (= (s (s (s z))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z))))) (& (= (s (s (s z))) (s (s (s z)))) (= (s (s (s (s (s z))))) (s (s (s (s (s z))))))))) (pair (wit (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z))))))) (inr (& (= (s (s (s z))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z))))) (pair (refl (s (s (s z)))) (refl (s (s (s (s (s z)))))))))"
filter_test "certify-gcd proves Euclid output divides both" "$SAMPLES/certify-gcd.alp" "12 8" "(& (Exists (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (Exists (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s z))))))))))) (pair (wit (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))) (s (s (s z))) (refl (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (wit (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s z))))))))) (s (s z)) (refl (s (s (s (s (s (s (s (s z)))))))))))"
filter_test "certify-triangle proves a loop sum = Gauss closed form" "$SAMPLES/certify-triangle.alp" "4" "(= (m (s (s z)) (s (s (s (s (s (s (s (s (s (s z))))))))))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (refl (m (s (s z)) (s (s (s (s (s (s (s (s (s (s z))))))))))))"
# The emitted code is overflow-SAFE: a compiled overflowing expr traps at runtime.
compiler_trap "exprc emits overflow trap" "$SAMPLES/exprc.alp" "46341*46341"
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
cat > "$T/mz.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) { let z: i32 = 0; let r: i32 = 7 % z; self.console.exit_process(r) }
EOF
trap_test "modulo by zero traps" "$T/mz.alp"
cat > "$T/af.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) { let x: i32 = 5; assert x > 100; self.console.exit_process(x) }
EOF
trap_test "false assert traps" "$T/af.alp"
cat > "$T/rq.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine need_positive(n: i32) -> i32 requires n > 0 { return n; }
machine Main::main(&mut self) { let r: i32 = need_positive(0); self.console.exit_process(r) }
EOF
trap_test "violated requires precondition traps" "$T/rq.alp"
cat > "$T/en.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine bad(n: i32) -> i32 ensures result > 1000 { return n; }
machine Main::main(&mut self) { let r: i32 = bad(5); self.console.exit_process(r) }
EOF
trap_test "violated ensures postcondition traps" "$T/en.alp"

# lowermachine array read: self.buf[i] -> bounds-checked element load
filter_test "lowermachine lowers a bounds-checked array read (self.buf[i])" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; buf: [u8; 256]; n: i32; } machine Main::main(&mut self) { self.n = self.buf[5]; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #256\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldrb w0, [x9]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #256]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine array store: self.buf[i] = v -> value+index pushed, bounds-checked address, pop value + strb/str
filter_test "lowermachine lowers a bounds-checked array store (self.buf[i] = v)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; buf: [u8; 256]; } machine Main::main(&mut self) { self.buf[5] = 9; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #9\n    str x0, [sp, #-16]!\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #256\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldr x1, [sp], #16\n    strb w1, [x9]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine large field offsets: scalar field past 16380 folds into x9 (mirrors backend add_imm_to_x9)
filter_test "lowermachine folds a scalar store past the 16380 ldr/str ceiling" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; buf: [u8; 20000]; n: i32; } machine Main::main(&mut self) { self.n = 5; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    add x9, x9, #4, lsl #12\n    add x9, x9, #3616\n    str w0, [x9]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine array offset >= 4096: hi<<12 + lo split
filter_test "lowermachine splits an array field offset >= 4096 (hi<<12 + lo)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; pad: [u8; 5000]; arr: [i32; 8]; } machine Main::main(&mut self) { self.arr[2] = 7; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #7\n    str x0, [sp, #-16]!\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #8\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, #1, lsl #12\n    add x9, x9, #904\n    add x9, x9, x0, lsl #3\n    ldr x1, [sp], #16\n    str w1, [x9]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine entry-by-name: main NOT first -> entry still gets _main + _selfdata, helper gets Lmachine0
filter_test "lowermachine emits the entry (_main) by name when it is not the first machine" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::helper(&mut self) { self.x = 1; transition 0 { _ -> h() } state h(){} } machine Main::main(&mut self) { self.helper(); transition 0 { _ -> s() } state s(){} }" "$(printf 'Lmachine0:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str x0, [x29, #16]\n    movz w0, #1\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x0, [x29, #16]\n    bl Lmachine0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine array count > 65535: bounds-check count splits movz w1,#lo + movk w1,#hi,lsl #16
filter_test "lowermachine splits an array count > 65535 (movz/movk in the bounds check)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; big: [u8; 70000]; } machine Main::main(&mut self) { self.big[5] = 9; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #9\n    str x0, [sp, #-16]!\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #4464\n    movk w1, #1, lsl #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldr x1, [sp], #16\n    strb w1, [x9]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine boundary call: write_byte(b) -> value + _iobyte/_write syscall sequence
filter_test "lowermachine lowers the write_byte boundary call (_iobyte + bl _write)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::main(&mut self) { write_byte(65); transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #65\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    adrp x9, _iobyte@PAGE\n    add x9, x9, _iobyte@PAGEOFF\n    strb w0, [x9]\n    mov x0, #1\n    adrp x1, _iobyte@PAGE\n    add x1, x1, _iobyte@PAGEOFF\n    mov x2, #1\n    bl _write\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine arithmetic operator: + -> adds w0,w0,w1 / b.vs Ltrap (was mis-lowered as a comparison)
filter_test "lowermachine lowers the + arithmetic operator (adds + overflow trap)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; y: i32; } machine Main::main(&mut self) { self.y = self.x + 5; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine multi-operator chain (left-assoc): a*10+c-48 emits three ops, not one
filter_test "lowermachine lowers a multi-operator chain left-associatively (a*10+c-48)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; lc: i32; c: i32; y: i32; } machine Main::main(&mut self) { self.y = self.lc * 10 + self.c - 48; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #10\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    smull x0, w0, w1\n    sxtw x2, w0\n    cmp x0, x2\n    b.ne Ltrap\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    movz w0, #48\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    subs w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine boundary call: read_byte() as an expression operand -> _read syscall + csel(EOF) + push
filter_test "lowermachine lowers read_byte() as a value-producing operand (_read + csel)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; c: i32; } machine Main::main(&mut self) { self.c = read_byte(); transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    mov x0, #0\n    adrp x1, _iobyte@PAGE\n    add x1, x1, _iobyte@PAGEOFF\n    mov x2, #1\n    bl _read\n    adrp x9, _iobyte@PAGE\n    add x9, x9, _iobyte@PAGEOFF\n    ldrb w1, [x9]\n    movz w2, #65535\n    movk w2, #65535, lsl #16\n    cmp x0, #1\n    csel w0, w1, w2, ge\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine qualified console call: self.console.exit_process(code) -> pop code to x0 + epilogue + ret
filter_test "lowermachine lowers self.console.exit_process (qualified call -> return-like exit)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::main(&mut self) { transition 0 { _ -> d() } state d(){ self.console.exit_process(0) } }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine write_line: Lstr<i> appearance-order index + len=content+1 (trailing newline) + bl _write block
filter_test "lowermachine lowers self.console.write_line (Lstr index + length)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::main(&mut self) { self.console.write_line(\"aa\"); self.console.write_line(\"bbbb\"); transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    mov x0, #1\n    adrp x1, Lstr0@PAGE\n    add x1, x1, Lstr0@PAGEOFF\n    movz w2, #3\n    bl _write\n    mov x0, #1\n    adrp x1, Lstr1@PAGE\n    add x1, x1, Lstr1@PAGEOFF\n    movz w2, #5\n    bl _write\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine operator precedence (shunting-yard): 16 + 8*lc binds as 16+(8*lc), not (16+8)*lc
filter_test "lowermachine respects operator precedence (16 + 8 * lc)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; lc: i32; y: i32; } machine Main::main(&mut self) { self.y = 16 + 8 * self.lc; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #16\n    str x0, [sp, #-16]!\n    movz w0, #8\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    smull x0, w0, w1\n    sxtw x2, w0\n    cmp x0, x2\n    b.ne Ltrap\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine parenthesized sub-expressions (precedence -1 sentinel on the operator stack): the emit_int mod idiom
filter_test "lowermachine lowers parenthesized sub-expressions (v - (v/10)*10)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; v: i32; y: i32; } machine Main::main(&mut self) { self.y = self.v - (self.v / 10) * 10; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #10\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cbz w1, Ltrap\n    sdiv w0, w0, w1\n    str x0, [sp, #-16]!\n    movz w0, #10\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    smull x0, w0, w1\n    sxtw x2, w0\n    cmp x0, x2\n    b.ne Ltrap\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    subs w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine empty-self: a struct with only the console boundary (data size 0) skips the _selfdata init
filter_test "lowermachine skips the self-init for an empty (field-less) struct" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; } machine Main::main(&mut self) { write_byte(65); transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    movz w0, #65\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    adrp x9, _iobyte@PAGE\n    add x9, x9, _iobyte@PAGEOFF\n    strb w0, [x9]\n    mov x0, #1\n    adrp x1, _iobyte@PAGE\n    add x1, x1, _iobyte@PAGEOFF\n    mov x2, #1\n    bl _write\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine WRAPPER: full object output -- header + bodies + Ltrap/brk + _selfdata .zerofill (no-I/O)
wrap_test "lowermachine emits the full object wrapper (header + Ltrap + _selfdata)" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::main(&mut self) { self.x = 5; transition 0 { _ -> s() } state s(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_selfdata,8,3')"
# lowermachine wrapper _iobyte: a program that uses write_byte gets the _iobyte .zerofill (usesio flag)
wrap_test "lowermachine emits _iobyte in the wrapper when host I/O is used" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine Main::main(&mut self) { write_byte(65); self.x = 5; transition 0 { _ -> s() } state s(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #65\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    adrp x9, _iobyte@PAGE\n    add x9, x9, _iobyte@PAGEOFF\n    strb w0, [x9]\n    mov x0, #1\n    adrp x1, _iobyte@PAGE\n    add x1, x1, _iobyte@PAGEOFF\n    mov x2, #1\n    bl _write\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_selfdata,8,3\n.zerofill __DATA,__bss,_iobyte,1,0')"
# lowermachine wrapper Lstr: write_line string literals -> __TEXT,__const + Lstr<i>: .byte tables (incl empty)
wrap_test "lowermachine emits the Lstr string tables in the wrapper" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; } machine Main::main(&mut self) { self.console.write_line(\"ab\"); self.console.write_line(\"\"); transition 0 { _ -> s() } state s(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    mov x0, #1\n    adrp x1, Lstr0@PAGE\n    add x1, x1, Lstr0@PAGEOFF\n    movz w2, #3\n    bl _write\n    mov x0, #1\n    adrp x1, Lstr1@PAGE\n    add x1, x1, Lstr1@PAGEOFF\n    movz w2, #1\n    bl _write\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_iobyte,1,0\n.section __TEXT,__const\n.align 2\nLstr0:\n    .byte 97,98,10\nLstr1:\n    .byte 10')"
# array field index: index lookup must not clobber the saved array count/eb
filter_test "lowermachine lowers an array store with a field index (a[self.i])" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; di: i32; dig: [i32; 16]; } machine Main::main(&mut self) { self.dig[self.di] = 1; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #1\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, #8\n    add x9, x9, x0, lsl #3\n    ldr x1, [sp], #16\n    str w1, [x9]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# array read as an operand: index sentinel keeps the outer + (5 + a[i])
filter_test "lowermachine keeps the outer operator when an array read is an operand" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; i: i32; y: i32; dig: [i32; 16]; } machine Main::main(&mut self) { self.y = 5 + self.dig[self.i]; transition 0 { _ -> s() } state s(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, #16\n    add x9, x9, x0, lsl #3\n    ldr w0, [x9]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# value-returning self-machine call in operand position: self.x = self.dbl(21)
filter_test "lowermachine lowers a value-returning self-call as an operand" "$SAMPLES/lowermachine.alp" "// SELF-HOSTING / CODEGEN: a COMPLETE transition -- fuses the subject lowering (lowersubj) with the
// arm dispatch (armdispatch). Phase 0 collects the field table (scalar fields, offset = 8*index),
// phase 1 the state labels (Lm<mi>s<si>), phase 2 lowers the first transition: the SUBJECT (a literal
// 'movz w0,#v' / push, OR a 'self.field' load 'ldr x9,[x29,#16]; ldr w0,[x9,#off]' / push) then the
// subject-pop and per-arm dispatch -- exactly the backend's Transition lowering (src/aarch64.rs):
//     ldr x0, [sp], #16              pop the subject
//   per value arm  (true=1, false=0, N):
//     movz w1, #<value> / cmp w0, w1 / b.eq Lm<mi>s<target>
//   per wild arm '_':
//     b Lm<mi>s<target>
// Combined with lowersubj (the subject), this is a complete transition. Single-machine inputs
// (mi 0) for verification; the target is resolved via the state table by name.
//   ... transition self.x { true -> a() false -> b() } state a(){} state b(){} ...
//     -> pop / movz w1,#1; cmp; b.eq Lm0s0 / movz w1,#0; cmp; b.eq Lm0s1
boundary trait Console {
    machine exit_process(return_code: i32);
    machine write_byte(b: i32);
    machine read_byte() -> i32;
    machine write_line(text: &[u8]);
} data Main { console: Console; x: i32; } machine Main::dbl(&mut self, c: i32) -> i32 { transition 0 { _ -> r() } state r(){ return c + c; } } machine Main::main(&mut self) { self.x = self.dbl(21); transition 0 { _ -> t() } state t(){} }" "$(printf 'Lmachine0:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w1, [x29, #16]\n    str x0, [x29, #24]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #21\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# THE SELF-HOSTING FIXPOINT: lowermachine.alp compiled by the backend (= lmx), run on its OWN
# source, must emit byte-for-byte the same .s the backend emits for that source. This is the
# closing of the delta arc -- the .alp compiler reproducing itself exactly.
DELTA_ARCH=aarch64 "$BIN" "$SAMPLES/lowermachine.alp" "$T/lmx" >/dev/null 2>"$T/err" \
  && "$T/lmx" < "$SAMPLES/lowermachine.alp" > "$T/self.s" 2>/dev/null
if cmp -s "$T/self.s" "$T/lmx.s"; then
# large constant materialization: literals >= 65536 split into movz w0,#lo + movk w0,#hi,lsl#16
wrap_test "lowermachine splits a large integer literal (100000) into movz+movk" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; r: i32; } machine Main::main(&mut self) { self.r = 100000; transition 0 { _ -> s() } state s(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #34464\n    movk w0, #1, lsl #16\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_selfdata,8,3')"
# newline-separated statements (no semicolons): write_line must not swallow the following statement
wrap_test "lowermachine keeps a statement that follows write_line with no semicolon" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; } machine Main::main(&mut self) { self.console.write_line(\"ab\") self.console.exit_process(0) }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    mov x0, #1\n    adrp x1, Lstr0@PAGE\n    add x1, x1, Lstr0@PAGEOFF\n    movz w2, #3\n    bl _write\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_iobyte,1,0\n.section __TEXT,__const\n.align 2\nLstr0:\n    .byte 97,98,10')"
# free machines (no &mut self): params in w0.. (no self reg shift), no self slot/store, bare-name call
wrap_test "lowermachine lowers a free machine (no self) and its bare-name call" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; } machine add2(a: i32, b: i32) -> i32 { transition 0 { _ -> s() } state s(){ return a + b; } } machine Main::main(&mut self) { self.x = add2(7, 19); transition 0 { _ -> t() } state t(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\nLmachine0:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w0, [x29, #16]\n    str w1, [x29, #24]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr w0, [x29, #24]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #7\n    str x0, [sp, #-16]!\n    movz w0, #19\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    bl Lmachine0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_selfdata,8,3')"
# value-call nested in an operator expression (2 * f(y-1)): call args get a -2 op-stack sentinel
wrap_test "lowermachine keeps the outer operator across a call with an operator-valued arg" "$SAMPLES/lowermachine.alp" "boundary trait Console { machine exit_process(return_code: i32); machine write_byte(b: i32); machine read_byte() -> i32; machine write_line(text: &[u8]); } data Main { console: Console; x: i32; y: i32; } machine dbl(n: i32) -> i32 { transition 0 { _ -> s() } state s(){ return n + n; } } machine Main::main(&mut self) { self.x = 2 * dbl(self.y - 1); transition 0 { _ -> t() } state t(){} }" "$(printf '// generated by the proof kernel-rs aarch64 backend (slices 1-2, 4-6)\n.global _main\n.align 2\nLmachine0:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str w0, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\n_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #2\n    str x0, [sp, #-16]!\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!\n    movz w0, #1\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    subs w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    bl Lmachine0\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    smull x0, w0, w1\n    sxtw x2, w0\n    cmp x0, x2\n    b.ne Ltrap\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLtrap:\n    brk #0x1\n.zerofill __DATA,__bss,_selfdata,16,3')"
  PASS=$((PASS+1)); echo "  ok  self-compile FIXPOINT: lowermachine.alp emits itself byte-identical"
else
  FAIL=$((FAIL+1)); echo "  FAIL self-compile fixpoint: not byte-identical"
fi

# The Delta-written compiler grows its logical source arena one checked cell at
# a time inside explicit byte backing. It must never accept a prefix after
# exhausting that backing.
set +e
dd if=/dev/zero bs=262145 count=1 2>/dev/null | "$T/lmx" > /dev/null 2>&1
source_overflow=$?
set -e
if [ "$source_overflow" = 2 ]; then
  PASS=$((PASS+1)); echo "  ok  lowermachine rejects exhausted logical source backing"
else
  FAIL=$((FAIL+1)); echo "  FAIL lowermachine source overflow: exit $source_overflow, expected 2"
fi

selfhost_file_test "self-hosting: lowermachine compiles the Omega0 bundle decoder" \
  "$SAMPLES/omega-bootstrap-bundle-decode.alp" "$T/bundle-ok"
selfhost_test "self-hosting: lowermachine preserves nested array contexts and keyword-named states" \
  "$SAMPLES/nested-array-context.alp" ""
selfhost_test "self-hosting: lowermachine parses both signed i32 literal boundaries" \
  "$SAMPLES/literal-boundaries.alp" ""

# The nested-array context stack has an explicit compiler resource ceiling.
# The adjacent admitted source must compile and run; the next context must
# reject deterministically rather than scanning beyond the retained source.
make_nested_array_context_source() {
  nested_count=$1
  nested_output=$2
  printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; values: [i32; 1]; result: i32; } machine Main::main(&mut self) { self.result = ' > "$nested_output"
  nested_index=0
  while [ "$nested_index" -lt "$nested_count" ]; do
    printf '%s' 'self.values[' >> "$nested_output"
    nested_index=$((nested_index + 1))
  done
  printf '0' >> "$nested_output"
  nested_index=0
  while [ "$nested_index" -lt "$nested_count" ]; do
    printf ']' >> "$nested_output"
    nested_index=$((nested_index + 1))
  done
  printf '%s' '; self.console.exit_process(self.result); }' >> "$nested_output"
}
make_nested_array_context_source 16 "$T/nested-array-16.alp"
selfhost_test "self-hosting: lowermachine admits exactly 16 nested array contexts" \
  "$T/nested-array-16.alp" ""
make_nested_array_context_source 17 "$T/nested-array-17.alp"
set +e
"$T/lmx" < "$T/nested-array-17.alp" > /dev/null 2>&1
nested_array_overflow=$?
set -e
if [ "$nested_array_overflow" = 3 ]; then
  PASS=$((PASS+1)); echo "  ok  lowermachine rejects the 17th nested array context"
else
  FAIL=$((FAIL+1)); echo "  FAIL lowermachine nested array overflow: exit $nested_array_overflow, expected 3"
fi

set +e
printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; values: [i32; 1]; result: i32; } machine Main::main(&mut self) { self.result = self.values[self.values[0]' | "$T/lmx" > /dev/null 2>&1
truncated_array_status=$?
printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; } machine Main::main(&mut self) { state machine' | "$T/lmx" > /dev/null 2>&1
trailing_machine_status=$?
set -e
if [ "$truncated_array_status" = 1 ] && [ "$trailing_machine_status" = 1 ]; then
  PASS=$((PASS+1)); echo "  ok  lowermachine rejects truncated array and keyword-state source"
else
  FAIL=$((FAIL+1)); echo "  FAIL lowermachine malformed-source guards: array=$truncated_array_status machine=$trailing_machine_status, expected 1/1"
fi

# SECOND-ORDER self-hosting: the byte-identical lowermachine ($T/lmx) compiles rpn.alp -- another
# real ~200-line Delta program (a shunting-yard compiler) -- and the resulting arm64 binary
# behaves identically to the trusted Rust-beta reference across the full operator range
# (precedence, parens, comparisons, modulo, and the bit ops), exercised end-to-end.
selfhost_test "self-hosting: lowermachine compiles rpn.alp; binary matches the reference" "$SAMPLES/rpn.alp" \
  "2 + 3" "2 + 3 * 4" "(1 + 2) * 3" "1 + 2 < 5" "8 >> 1 & 3" "10 % 3 + 1"
# calc.alp: a recursive-descent evaluator -- exercises mutual recursion (factor/expr/term), parens,
# and let-LOCAL accumulation reassigned ACROSS states (acc = acc + self.term()). The last needed the
# bare-ident local-assignment statement path (ck2wb -> lasgn0); without it lowermachine dropped the
# stores and returned the let-initializer.
selfhost_test "self-hosting: lowermachine compiles calc.alp; binary matches the reference" "$SAMPLES/calc.alp" \
  "2+3*4" "(2+3)*4" "100/5/2" "((1+2))*3" "7-3-2"
# assert.alp: the dynamic-contract feature (`assert <cond>;` traps on false). Needed the assert
# statement path (ckas -> asrt0 -> mode 9 -> cbz w0, Ltrap). All asserts hold -> exit 42.
selfhost_test "self-hosting: lowermachine compiles assert.alp; binary matches the reference" "$SAMPLES/assert.alp" ""
selfhost_test "self-hosting: lowermachine lowers i32 in Wrapping (no overflow trap); matches reference" "$SAMPLES/wraptest.alp" ""
selfhost_test "self-hosting: lowermachine lowers i32 in Saturating (clamp to MIN/MAX); matches reference" "$SAMPLES/sattest.alp" ""
selfhost_test "self-hosting: lowermachine lowers a Saturating FIELD store (per-field fdom table); matches reference" "$SAMPLES/fieldsat.alp" ""
# lowerexpr.alp: itself a COMPILER (emits arm64 asm). lowermachine compiling it exercises a
# self-method-call statement with no trailing ';' as a block's last statement (state pu(){self.push()});
# without the callsk '}'-stop the post-call skip ran past the next machine, breaking _main.
selfhost_test "self-hosting: lowermachine compiles lowerexpr.alp (a compiler); emitted asm matches reference" "$SAMPLES/lowerexpr.alp" \
  "2 + 3" "2 + 3 * 4" "(1 + 2) * 3" "8 >> 1 & 3"
# enum.alp: tag-only sum types (data E { case A; case B; }). lowermachine needed a variant->tag
# table (pass-1 'case' parsing) + Type::Variant as a tag literal in operand (construction) AND arm
# (matching) positions. Color::Green (tag 1) -> green() -> exit 7.
selfhost_test "self-hosting: lowermachine compiles enum.alp (tag-only sum types); matches reference" "$SAMPLES/enum.alp" ""
# stateargs.alp: state PARAMETERS -- an arm `_ -> show(EXPR)` evaluates EXPR and stores it into the
# target state's frame slot before branching; the state reads it by name. The prerequisite for `case`
# payload binding. 6 * 7 -> show(42) -> exit 42. (Needed addS2->ckspar param parsing + mode-10 arg store.)
selfhost_test "self-hosting: lowermachine compiles stateargs.alp (state parameters); matches reference" "$SAMPLES/stateargs.alp" ""
# statedispatch.alp: tag-matched arm with a state arg -- `N -> s(EXPR)` lowers as cmp/b.ne Lsk/store/b
# target/Lsk: (the arg stored only on match). The eval-arm half of state-args; payload arms reuse it.
selfhost_test "self-hosting: lowermachine compiles statedispatch.alp (eval-arm state args); matches reference" "$SAMPLES/statedispatch.alp" ""
# payloadctor.alp: enum payload construction + field sizing -- `self.sh = Shape::Square(36)` stores
# tag at sh+0, payload at sh+8; the enum field is sized to tag+widest-payload (.zerofill 16). Bare-arm
# tag dispatch -> Square -> b -> exit 2. (Needed field tables bumped 128->256 for lowermachine's own struct.)
selfhost_test "self-hosting: lowermachine compiles payloadctor.alp (payload construction + sizing); matches reference" "$SAMPLES/payloadctor.alp" ""
# payload.alp: the full single-payload enum round-trip -- a match arm `Shape::Square { s } -> got(s + 6)`
# binds s to the subject's payload (read of subject-field + 8) and passes s + 6 to a state param.
# Square(36) -> got(42) -> exit 42. Completes the single-payload sum-type system.
selfhost_test "self-hosting: lowermachine compiles payload.alp (payload binding); matches reference" "$SAMPLES/payload.alp" ""
# shape.alp: MULTI-FIELD payloads -- `case Rectangle(w: i32, h: i32)` (enum sized to the widest variant,
# 24 bytes); construction stores each field at +8, +16, ...; the arm `Rectangle { w, h } -> area(w*h)`
# binds each name to its field. Rectangle(6,7) -> area(42) -> exit 42. Completes the sum-type system.
selfhost_test "self-hosting: lowermachine compiles shape.alp (multi-field payloads); matches reference" "$SAMPLES/shape.alp" ""
# negate.alp: unary minus on a primary -- `-7`, `-a`, `2 * -3`, `10 - -5`. lowered as: lower the
# following primary, then `neg w0`. Found by the selfhost run-compare sweep. -(-7)+ (2*-3+20) + (10--5) = 36.
selfhost_test "self-hosting: lowermachine compiles negate.alp (unary minus); matches reference" "$SAMPLES/negate.alp" ""
# minmax.alp: min(a,b) / max(a,b) as OPERAND-STACK sentinels (prec -4/-5, below the -3 paren sentinel) --
# NOT the call machinery (whose single-field argc/cmi clobber across nesting). '(' after min/max pushes the
# sentinel; ',' drains arg1's ops to it then lowers arg2; ')' pops it and emits `cmp w0,w1; csel {lt|gt}`.
# Covers the nested clamp idiom max(0, min(x, hi)). Ceiling-fix (buf 262144 / cap 1024*256) unblocked this.
selfhost_test "self-hosting: lowermachine compiles minmax.alp (min/max operand-stack sentinels); matches reference" "$SAMPLES/minmax.alp" ""
# stateparams.alp: MULTI-param states -- `state go(i: i32, acc: i32)` + `go(0,1)` / `go(i+1, acc*2)`.
# Each param is a consecutive frame local; an arm `go(a, b)` stores arg j to the (first+j)-th slot,
# looping by source ',' / ')'. Recursive doubling go(0..3) -> fin(16) -> exit 16. (Sweep DIFF.)
selfhost_test "self-hosting: lowermachine compiles stateparams.alp (multi-param states); matches reference" "$SAMPLES/stateparams.alp" ""
# loop.alp: BARE `write_line("tick")` (not self.console.write_line) in a back-edge loop. lowermachine
# handled the qualified call + bare write_byte/read_byte but not bare write_line -> the call was dropped.
# Fix: ck2wb-false -> ck2wl (is_wline) -> the existing wline string emitter. Prints tick x3, exit 3. (Sweep DIFF.)
selfhost_test "self-hosting: lowermachine compiles loop.alp (bare write_line); matches reference" "$SAMPLES/loop.alp" ""
# a FALSE assert in a lowermachine-compiled program must TRAP at runtime (cbz w0, Ltrap fires).
compiler_trap "lowermachine compiles a false assert into a runtime trap" "$SAMPLES/lowermachine.alp" \
  "boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; } machine Main::main(&mut self) { let a: i32 = 0; assert a > 5; self.console.exit_process(7) }"

echo "aarch64 macOS backend gate (slices 1-7, full parity): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
