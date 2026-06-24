#!/usr/bin/env sh
# Gate for bc.beta — the Beta compiler written in Beta. Builds bc (via the Rust
# on-ramp), then uses bc AS a compiler: feeds it whole `proc main() { ... }`
# programs, assembles + runs bc's emitted asm, and checks the exit code.
#   slice 1  : arithmetic (return <expr>)
#   slice 2a : + let locals, assignment, variable references
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED

# build bc.exe = the bc.beta compiler, lowered through the on-ramp
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
BC=../beta-lang-rs/build/bc.exe
echo "bc tape: $(wc -c < ../beta-lang-rs/build/bc.tape | tr -d ' ') B (hole $HOLE_SIZE)"

PASS=0; FAIL=0
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
ret() { # full-program  expected   (compile + assemble + run, check exit code)
  printf '%s\n' "$1" | "$BC" > "$T/p.asm" 2>/dev/null
  "$ASM" < "$T/p.asm" > "$T/p.tape" 2>/dev/null
  stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1
  "$T/p.exe" </dev/null; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $2 got $got : $1"; fi
}
expr() { ret "proc main() { return $1 }" "$2"; }   # arithmetic shorthand
# slice 1 — arithmetic
expr "6 * 7" 42
expr "2 + 3 * 4" 14
expr "(2 + 3) * 4" 20
expr "100 - 58" 42
expr "2 * (3 + 4) * 5" 70
expr "100 / 7" 14
expr "17 % 5" 2
expr "1 + 2 + 3 + 4 + 5" 15
# slice 2a — locals, assignment, variable references
ret "proc main() { let a = 6 let b = 7 return a * b }" 42
ret "proc main() { let x = 10 let y = x * x return y - x }" 90
ret "proc main() { let a = 2 let b = 3 let c = 4 return a + b * c }" 14
ret "proc main() { let n = 5 let s = 0 s = s + n s = s + n return s }" 10
ret "proc main() { let a = 100 a = a - 58 return a }" 42
# slice 2b — if/else, while, the six comparisons
ret "proc main() { let a = 5 if a < 10 { return 1 } return 0 }" 1
ret "proc main() { let a = 5 if a > 10 { return 1 } return 0 }" 0
ret "proc main() { let a = 5 if a == 5 { return 42 } else { return 7 } }" 42
ret "proc main() { let a = 9 if a == 5 { return 42 } else { return 7 } }" 7
ret "proc main() { let i = 0 let s = 0 while i < 5 { s = s + i i = i + 1 } return s }" 10
ret "proc main() { let n = 10 let s = 0 let i = 1 while i <= n { s = s + i i = i + 1 } return s }" 55
ret "proc main() { let a = 3 if a != 3 { return 1 } if a >= 3 { return 99 } return 0 }" 99
# slice 3 — procedures, parameters, calls, recursion
ret "proc main() { return double(21) } proc double(x) { return x + x }" 42
ret "proc main() { return add(mul(2, 3), 4) } proc add(a, b) { return a + b } proc mul(a, b) { return a * b }" 10
ret "proc main() { return fact(5) } proc fact(n) { if n < 2 { return 1 } return n * fact(n - 1) }" 120
ret "proc main() { return fib(10) } proc fib(n) { if n < 2 { return n } return fib(n - 1) + fib(n - 2) }" 55
ret "proc main() { return sumto(10) } proc sumto(n) { let t = 0 let i = 1 while i <= n { t = t + i i = i + 1 } return t }" 55
# slice 4 — byte[]/word[] memory
ret "proc main() { let b = 2097152 byte[b] = 65 byte[b + 1] = 66 return byte[b] + byte[b + 1] }" 131
ret "proc main() { let base = 2097152 let i = 0 while i < 5 { word[base + i * 8] = i * i i = i + 1 } let t = 0 i = 0 while i < 5 { t = t + word[base + i * 8] i = i + 1 } return t }" 30
# slice 5 — char literals (intrinsics/emit/IO are covered end-to-end by selfhost.sh)
ret "proc main() { return 'A' }" 65
ret "proc main() { return '0' + 9 }" 57
ret "proc main() { let c = 'Z' if c == 90 { return 42 } return 0 }" 42
ret "proc main() { return f(10, 20, 30) } proc f(a, b, c) { return a + c }" 40
ret "proc main() { return g(1, 2, 3, 4) } proc g(a, b, c, d) { return a + b + c + d }" 10
echo "bc.beta (slices 1-6, per-feature): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
