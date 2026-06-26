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
tokens_test "tokenize multi-char ops" samples/tokenize.alp "Main::main n<=3 a->b x==y i-1" "Main :: main n <= 3 a -> b x == y i - 1"
# balance: a bracket-balance validator for .alp (typed stack; skips comments/strings).
stdin_exit "balance ok (nested/mixed + comment/string)" samples/balance.alp 'f(){ a[0]=1; } // ) ]
x="}"' 0
stdin_exit "balance bad (type mismatch)" samples/balance.alp 'f( ]' 1
# decls: keyword recognition + structure extraction — list .alp machine names.
tokens_test "decls lists machine names" samples/decls.alp "machine max(a,b){} machine Main::main(){}" "max Main"
tokens_test "decls keyword vs prefix (machinery != machine)" samples/decls.alp "machinery x; machine f(){}" "f"
tokens_test "decls lists data + machine names" samples/decls.alp "data Pt{x:i32;} machine f(){}" "Pt f"
tokens_test "decls keyword vs prefix (database != data)" samples/decls.alp "database x; data D{} machine M(){}" "D M"
# certify-add emits a delta proof certificate for a+b (verified end-to-end by
# convergence.sh; here just check the epsilon side produces the exact certificate).
filter_test "certify-add emits a delta cert" samples/certify-add.alp "2 3" "(= (p (s (s z)) (s (s (s z)))) (s (s (s (s (s z)))))) (refl (s (s (s (s (s z))))))"
filter_test "certify-lt emits an existential-witness cert" samples/certify-lt.alp "2 5" "(Exists (= (p (s (s z)) (s (v 0))) (s (s (s (s (s z))))))) (wit (= (p (s (s z)) (s (v 0))) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z)))))))"
filter_test "certify-bounds emits a 2D array-bounds VC" samples/certify-bounds.alp "2 5 3 4" "(Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z))))))))"
filter_test "certify-divides emits a divisibility cert" samples/certify-divides.alp "3 12" "(Exists (= (m (v 0) (s (s (s z)))) (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (wit (= (m (v 0) (s (s (s z)))) (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))) (s (s (s (s z)))) (refl (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))))"
filter_test "certify-accesses emits a whole-program safety proof" samples/certify-accesses.alp "2 5 3 4  1 3 0 2" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))) (s (s z)) (refl (m (s (s z)) (s (s (s z)))))))"
filter_test "certify-safety emits a mixed bounds+nonzero proof" samples/certify-safety.alp "b 2 5 3 4  d 7  b 1 3 0 2" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (& (Exists (= (p z (s (v 0))) (s (s (s (s (s (s (s z))))))))) (Exists (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z)))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (pair (wit (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))) (s (s (s (s (s (s z)))))) (refl (s (s (s (s (s (s (s z))))))))) (wit (= (p (p (m (s z) (s (s (s z)))) z) (s (v 0))) (m (s (s z)) (s (s (s z))))) (s (s z)) (refl (m (s (s z)) (s (s (s z))))))))"
filter_test "certify-source compiles source to a safety proof" samples/certify-source.alp "arr 4 5  get 2 3  div 7" "(& (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))))) (pair (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s z)))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p z (s (v 0))) (s (s (s (s (s (s (s z)))))))) (s (s (s (s (s (s z)))))) (refl (s (s (s (s (s (s (s z))))))))))"
stdin_exit "certify-source rejects unsafe access (exit 1)" samples/certify-source.alp "arr 4 5  get 3 7" 1
stdin_exit "certify-source accepts safe code (exit 0)" samples/certify-source.alp "arr 4 5  get 2 3" 0
# statecheck: a real .alp frontend pass (name resolution) written in .alp
stdin_exit "statecheck accepts clean .alp (exit 0)" samples/statecheck.alp "machine M::m(){ state a(){ transition 0 { _ -> a() } } }" 0
stdin_exit "statecheck rejects an undefined transition target (exit 1)" samples/statecheck.alp "machine M::m(){ state a(){ transition 0 { _ -> zz() } } }" 1
# dupcheck: duplicate-state detection, machine-SCOPED (a real .alp frontend pass in .alp)
stdin_exit "dupcheck accepts unique states (exit 0)" samples/dupcheck.alp "machine M::m(){ state a(){} state b(){} }" 0
stdin_exit "dupcheck rejects a duplicate state (exit 1)" samples/dupcheck.alp "machine M::m(){ state a(){} state a(){} }" 1
stdin_exit "dupcheck is machine-scoped (same name diff machines, exit 0)" samples/dupcheck.alp "machine A::a(){ state s(){} } machine B::b(){ state s(){} }" 0
# unreachable: dead-state (orphan) detection, machine-scoped -- converse of statecheck
stdin_exit "unreachable accepts all-reached states (exit 0)" samples/unreachable.alp "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> a() } } }" 0
stdin_exit "unreachable flags an orphan/dead state (exit 1)" samples/unreachable.alp "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> a() } } state dead(){} }" 1
# fieldcheck: member name resolution -- every self.X field access must be a declared field
stdin_exit "fieldcheck accepts valid fields (exit 0)" samples/fieldcheck.alp "data Main{ x: i32; } machine A::a(&mut self){ state s(){ self.x = 1; } }" 0
stdin_exit "fieldcheck flags an unknown field (exit 1)" samples/fieldcheck.alp "data Main{ x: i32; } machine A::a(&mut self){ state s(){ self.y = 1; } }" 1
# methodcheck: method-call name resolution -- every self.m() call names a declared machine method
stdin_exit "methodcheck accepts a declared method call (exit 0)" samples/methodcheck.alp "machine A::a(&mut self){ state s(){ self.b(); } } machine A::b(&mut self){ state t(){} }" 0
stdin_exit "methodcheck flags an unknown method call (exit 1)" samples/methodcheck.alp "machine A::a(&mut self){ state s(){ self.b(); } }" 1
# layout: data-struct field-offset computation (BUILDING, not checking) -- reproduces the backend layout
filter_test "layout computes data-struct field offsets (Console=0, i32=8, [u8;N]=N)" samples/layout.alp "boundary trait C{} data Main{ c: C; n: i32; buf: [u8; 16]; m: i32; }" "$(printf 'c 0\nn 0\nbuf 8\nm 24')"
# cfg: control-flow-graph edge emission (BUILDING) -- from->target edges, enclosing-state tracked
filter_test "cfg emits control-flow graph edges (from target, machine-scoped)" samples/cfg.alp "machine M::m(){ transition 0 { _ -> a() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf 'entry a\na b')"
# labels: state -> Lm<mi>s<si> assignment (BUILDING) -- reproduces the backend label scheme EXACTLY
filter_test "labels assigns backend state labels (Lm<mi>s<si>, depth-aware)" samples/labels.alp "machine A::a(){ state p(){} state q(){} } machine B::b(){ state r(){} }" "$(printf 'p Lm0s0\nq Lm0s1\nr Lm1s0')"
# branches: CODEGEN -- lower transition arms to b/b.eq Lm<mi>s<si> with target resolution
filter_test "branches lowers transition arms to resolved b/b.eq labels" samples/branches.alp "machine M::m(){ transition 0 { _ -> a() } state a(){ transition self.x { true -> b() false -> a() } } state b(){} }" "$(printf 'b Lm0s0\nb.eq Lm0s1\nb.eq Lm0s0')"
# armdispatch: CODEGEN -- the arm-DISPATCH half of a transition (subject-pop + per-arm cmp/branch)
filter_test "armdispatch lowers a transition's arm dispatch (pop + value-arm cmp/b.eq)" samples/armdispatch.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1')"
# lowertrans: CODEGEN -- a COMPLETE transition (subject lowering fused with arm dispatch)
filter_test "lowertrans lowers a complete transition (self.field subject + pop + arms)" samples/lowertrans.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1')"
# lowerbody: CODEGEN -- a whole MACHINE BODY (per-state labels + every transition lowered)
filter_test "lowerbody emits the machine control skeleton (labels + all transitions)" samples/lowerbody.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:')"
# lowermachine: CODEGEN -- THE ORCHESTRATOR: a complete machine (scaffold wrapped around lowerbody)
filter_test "lowermachine emits a complete machine (_main: prologue + body + epilogue)" samples/lowermachine.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){ transition 0 { _ -> b() } } state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine operator subject: a comparison subject (cmp + cset) lowered inside the complete machine
filter_test "lowermachine lowers a comparison subject (self.x < 0 -> cmp/cset)" samples/lowermachine.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x < 0 { true -> a() false -> b() } state a(){} state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, lt\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine assignment statements: self.field = EXPR; lowered as RHS + storefield, inside the machine
filter_test "lowermachine lowers assignment statements (self.field = EXPR -> RHS + storefield)" samples/lowermachine.alp "data Main { x: i32; y: i32; } machine Main::main(&mut self) { self.x = 5; transition 0 { _ -> a() } state a(){ self.y = self.x; transition 0 { _ -> b() } } state b(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s1\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine locals: let declarations size the frame (align16(24+8*lc), self_disp 16+8*lc) + let-store
filter_test "lowermachine sizes the frame for locals and lowers let-store" samples/lowermachine.alp "data Main { y: i32; } machine Main::main(&mut self) { let x: i32 = 5; self.y = 9; transition 0 { _ -> a() } state a(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #24]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    str w0, [x29, #16]\n    movz w0, #9\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #24]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine local-read: a local used as a value -> ldr w0,[x29,#disp] + push
filter_test "lowermachine reads a local as an operand (let x; self.y = x)" samples/lowermachine.alp "data Main { y: i32; } machine Main::main(&mut self) { let x: i32 = 5; self.y = x; transition 0 { _ -> a() } state a(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #24]\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    str w0, [x29, #16]\n    ldr w0, [x29, #16]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    ldr x9, [x29, #24]\n    str w0, [x9, #0]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm0s0\nLm0s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# lowermachine multi-machine: emits each machine with its own scaffold (entry _main loads _selfdata; others Lmachine<mi> take self in x0)
filter_test "lowermachine emits multiple machines (entry _main + Lmachine1)" samples/lowermachine.alp "data Main { x: i32; } machine Main::main(&mut self) { transition self.x { true -> a() false -> b() } state a(){} state b(){} } machine Main::helper(&mut self) { transition 0 { _ -> h() } state h(){} }" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    ldr x9, [x29, #16]\n    ldr w0, [x9, #0]\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    movz w1, #1\n    cmp w0, w1\n    b.eq Lm0s0\n    movz w1, #0\n    cmp w0, w1\n    b.eq Lm0s1\nLm0s0:\nLm0s1:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret\nLmachine1:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    str x0, [x29, #16]\n    movz w0, #0\n    str x0, [sp, #-16]!\n    ldr x0, [sp], #16\n    b Lm1s0\nLm1s0:\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# sizeof: align8 total struct size (frame size) -- matches the backend _selfdata directive
filter_test "sizeof emits align8 total struct size" samples/sizeof.alp "boundary trait C{} data Main{ c: C; n: i32; buf: [u8; 13]; }" "24"
# rpn: infix -> RPN (shunting-yard) -- the expression-lowering arc, step 1 (linearize to stack-machine order)
filter_test "rpn linearizes an expression to postfix (precedence + assoc)" samples/rpn.alp "e - s == 5" "$(printf 'e\ns\n-\n5\n==')"
filter_test "rpn handles array index (self.buf[i] -> index RPN then base[])" samples/rpn.alp "self.buf[s] == 115" "$(printf 's\nself.buf[]\n115\n==')"
# loadk: ARM64 constant materialization (movz/movk) -- first asm-emitting expression primitive
filter_test "loadk emits ARM64 constant load (movz + movk high half)" samples/loadk.alp "100000" "$(printf 'movz w0, #34464\nmovk w0, #1, lsl #16')"
# lowerop: binary-operator ARM64 snippets (static half of expression lowering)
filter_test "lowerop emits the backend binary-op snippet (<= -> cmp/cset le)" samples/lowerop.alp "<=" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, le\n    str x0, [sp, #-16]!')"
# fieldload: self.field operand lowering -- the symbol-table half (field name -> layout offset -> SelfField load)
filter_test "fieldload lowers a self.field read to its backend SelfField load" samples/fieldload.alp "data Main { a: i32; b: i32; c: i32; } machine Main::main(&mut self) { transition self.b { _ -> h() } state h(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!')"
# lowerexpr: the ORCHESTRATOR -- lower a complete expression end-to-end (rpn + loadk + lowerop composed)
filter_test "lowerexpr lowers a full expression (2 + 3) to the backend instruction sequence" samples/lowerexpr.alp "2 + 3" "$(printf '    movz w0, #2\n    str x0, [sp, #-16]!\n    movz w0, #3\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    adds w0, w0, w1\n    b.vs Ltrap\n    str x0, [sp, #-16]!')"
# lowersubj: lower a real transition subject (literals + self.field operands + operators)
filter_test "lowersubj lowers a self.field transition subject end-to-end" samples/lowersubj.alp "data Main { a: i32; b: i32; c: i32; } machine Main::main(&mut self) { transition self.b < 5 { _ -> h() } state h(){} }" "$(printf '    ldr x9, [x29, #16]\n    ldr w0, [x9, #8]\n    str x0, [sp, #-16]!\n    movz w0, #5\n    str x0, [sp, #-16]!\n    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    cmp w0, w1\n    cset w0, lt\n    str x0, [sp, #-16]!')"
# arrindex: array-index operand (SelfIndex) -- bounds-checked element load, the last operand kind
filter_test "arrindex emits the backend SelfIndex sequence (u8 array, offset 0 count 16)" samples/arrindex.alp "0 16 1" "$(printf '    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldrb w0, [x9]\n    str x0, [sp, #-16]!')"
# storefield: StoreSelfField (the store half of an assignment) -- statement lowering begins
filter_test "storefield emits the backend field-store (offset 8)" samples/storefield.alp "8" "$(printf '    ldr x0, [sp], #16\n    ldr x9, [x29, #16]\n    str w0, [x9, #8]')"
# storeindex: StoreSelfIndex (array element store) -- self.arr[i] = value
filter_test "storeindex emits the backend array-store (u8 array, offset 0 count 16)" samples/storeindex.alp "0 16 1" "$(printf '    ldr x0, [sp], #16\n    movz w1, #16\n    cmp w0, w1\n    b.hs Ltrap\n    uxtw x0, w0\n    ldr x9, [x29, #16]\n    add x9, x9, x0\n    ldr x1, [sp], #16\n    strb w1, [x9]')"
# scaffold: the machine FRAME (prologue + trailing default + epilogue) for the entry _main
filter_test "scaffold emits the entry-machine frame (local_count 0 -> frame 32)" samples/scaffold.alp "0" "$(printf '_main:\n    sub sp, sp, #32\n    stp x29, x30, [sp]\n    mov x29, sp\n    adrp x9, _selfdata@PAGE\n    add x9, x9, _selfdata@PAGEOFF\n    str x9, [x29, #16]\n    mov w0, #0\n    mov sp, x29\n    ldp x29, x30, [sp]\n    add sp, sp, #32\n    ret')"
# selfcall: method call (SelfCall) -- pop args, self in x0, bl callee, push return
filter_test "selfcall emits the backend method-call sequence (1 arg)" samples/selfcall.alp "1 Lmachine3" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [x29, #16]\n    bl Lmachine3\n    str x0, [sp, #-16]!')"
# freecall: free-machine call (Call) -- args x0.., no self, bl callee, push
filter_test "freecall emits the backend free-call sequence (2 args)" samples/freecall.alp "2 Lmachine5" "$(printf '    ldr x1, [sp], #16\n    ldr x0, [sp], #16\n    bl Lmachine5\n    str x0, [sp, #-16]!')"
# layout regression: a last field with no trailing semicolon (data ... i32 }) must still parse
filter_test "layout handles last field without trailing semicolon" samples/layout.alp "boundary trait C{} data Main{ c: C; n: i32 }" "$(printf 'c 0\nn 0')"
stdin_exit "certify-source rejects an overrunning loop (exit 1)" samples/certify-source.alp "arr 4 5  band 5 0" 1
filter_test "certify-source unrolls a bounded loop (band)" samples/certify-source.alp "arr 4 5  band 3 0" "(& (Exists (= (p (p (m z (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (& (Exists (= (p (p (m (s z) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))))) (pair (wit (= (p (p (m z (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (pair (wit (= (p (p (m (s z) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (wit (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (s (s (s (s (s (s (s (s (s z))))))))) (refl (m (s (s (s (s z)))) (s (s (s (s (s z))))))))))"
filter_test "certify-linked emits a lemma-citing proof" samples/certify-linked.alp "2 5 3 4" "(Exists (= (p (p (m (s (s z)) (s (s (s (s (s z)))))) (s (s (s z)))) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))) (app (app (inst (inst (inst (inst (use 30) (s (s z))) (s (s (s (s (s z)))))) (s (s (s z)))) (s (s (s (s z))))) (wit (= (p (s (s z)) (s (v 0))) (s (s (s (s z))))) (s z) (refl (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (s (v 0))) (s (s (s (s (s z)))))) (s z) (refl (s (s (s (s (s z))))))))"
filter_test "certify-loop emits a forall-loop proof citing two lemmas" samples/certify-loop.alp "4 5 0 4" "(All (-> (Exists (= (p (v 1) (s (v 0))) (s (s (s (s z)))))) (Exists (= (p (p (m (v 1) (s (s (s (s (s z)))))) z) (s (v 0))) (m (s (s (s (s z)))) (s (s (s (s (s z)))))))))) (gen (lam (Exists (= (p (v 1) (s (v 0))) (s (s (s (s z)))))) (app (app (inst (inst (inst (inst (use 30) (v 0)) (s (s (s (s (s z)))))) z) (s (s (s (s z))))) (app (app (inst (inst (inst (use 34) (v 0)) (s (s (s (s z))))) (s (s (s (s z))))) (hyp 0)) (wit (= (p (s (s (s (s z)))) (v 0)) (s (s (s (s z))))) z (refl (s (s (s (s z)))))))) (wit (= (p z (s (v 0))) (s (s (s (s (s z)))))) (s (s (s (s z)))) (refl (s (s (s (s (s z))))))))))"
filter_test "certify-mul cites the overflow theorem" samples/certify-mul.alp "3 4 5 6" "(Exists (= (p (m (s (s (s z))) (s (s (s (s z))))) (s (v 0))) (m (s (s (s (s (s z))))) (s (s (s (s (s (s z))))))))) (app (app (inst (inst (inst (inst (use 66) (s (s (s z)))) (s (s (s (s z))))) (s (s (s (s (s z)))))) (s (s (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (s (v 0))) (s (s (s (s (s z)))))) (s z) (refl (s (s (s (s (s z)))))))) (wit (= (p (s (s (s (s z)))) (s (v 0))) (s (s (s (s (s (s z))))))) (s z) (refl (s (s (s (s (s (s z)))))))))"
filter_test "certify-max proves its result meets the max spec" samples/certify-max.alp "5 3" "(& (& (Exists (= (p (s (s (s (s (s z))))) (v 0)) (s (s (s (s (s z))))))) (Exists (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))))) (+ (= (s (s (s (s (s z))))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z)))))) (pair (pair (wit (= (p (s (s (s (s (s z))))) (v 0)) (s (s (s (s (s z)))))) z (refl (s (s (s (s (s z))))))) (wit (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z)))))))) (inl (= (s (s (s (s (s z))))) (s (s (s z)))) (refl (s (s (s (s (s z))))))))"
filter_test "certify-sort2 proves ordered + permutation" samples/certify-sort2.alp "5 3" "(& (Exists (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z))))))) (+ (& (= (s (s (s z))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z))))) (& (= (s (s (s z))) (s (s (s z)))) (= (s (s (s (s (s z))))) (s (s (s (s (s z))))))))) (pair (wit (= (p (s (s (s z))) (v 0)) (s (s (s (s (s z)))))) (s (s z)) (refl (s (s (s (s (s z))))))) (inr (& (= (s (s (s z))) (s (s (s (s (s z)))))) (= (s (s (s (s (s z))))) (s (s (s z))))) (pair (refl (s (s (s z)))) (refl (s (s (s (s (s z)))))))))"
filter_test "certify-gcd proves Euclid output divides both" samples/certify-gcd.alp "12 8" "(& (Exists (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (Exists (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s z))))))))))) (pair (wit (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s (s (s (s (s z))))))))))))) (s (s (s z))) (refl (s (s (s (s (s (s (s (s (s (s (s (s z)))))))))))))) (wit (= (m (v 0) (s (s (s (s z))))) (s (s (s (s (s (s (s (s z))))))))) (s (s z)) (refl (s (s (s (s (s (s (s (s z)))))))))))"
filter_test "certify-triangle proves a loop sum = Gauss closed form" samples/certify-triangle.alp "4" "(= (m (s (s z)) (s (s (s (s (s (s (s (s (s (s z))))))))))) (m (s (s (s (s z)))) (s (s (s (s (s z))))))) (refl (m (s (s z)) (s (s (s (s (s (s (s (s (s (s z))))))))))))"
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
