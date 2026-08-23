#!/usr/bin/env sh
# Gate for bc.beta — the Beta compiler written in Beta. Builds bc (via the Rust
# on-ramp), then uses bc AS a compiler: feeds it whole `proc main() { ... }`
# programs, assembles + runs bc's emitted asm, and checks the exit code.
#   slice 1  : arithmetic (return <expr>)
#   slice 2a : + let locals, assignment, variable references
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED

# build bc.exe = the bc.beta compiler, lowered through the on-ramp
( cd "${OMEGA_PATH_BETA_COMPILER_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
BC="${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.exe
echo "bc tape: $(wc -c < "${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.tape | tr -d ' ') B (hole $HOLE_SIZE)"

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
# slice 2b — the six comparisons via CFG guards (Beta has no if/while)
ret "proc main() { let a = 5 state s { to y when (a < 10) return 0 } state y { return 1 } }" 1
ret "proc main() { let a = 5 state s { to y when (a > 10) return 0 } state y { return 1 } }" 0
ret "proc main() { let a = 5 state s { to y when (a == 5) return 7 } state y { return 42 } }" 42
ret "proc main() { let a = 9 state s { to y when (a == 5) return 7 } state y { return 42 } }" 7
ret "proc main() { let a = 3 state s1 { to bad when (a != 3) to s2 } state bad { return 1 } state s2 { to y when (a >= 3) return 0 } state y { return 99 } }" 99
# slice 2c — CFG / Omega-style control flow: states + guarded transitions
ret "proc main() { state s0 { let x = 7 to done when (x == 7) return 0 } state done { return 42 } }" 42
ret "proc main() { state e { let s = 0 let i = 10 to loop } state loop { to fin when (i == 0) s = s + i i = i - 1 to loop } state fin { return s } }" 55
ret "proc main() { let n = 10 let s = 0 let i = 1 state l { to b when (i <= n) return s } state b { s = s + i i = i + 1 to l } }" 55
ret "proc main() { let i = 0 let s = 0 state l { to b when (i < 5) return s } state b { s = s + i i = i + 1 to l } }" 10
ret "proc main() { return countdown(5) } proc countdown(n) { state go { to done when (n == 0) n = n - 1 to go } state done { return n } }" 0
# slice 3 — procedures, parameters, calls, recursion
ret "proc main() { return double(21) } proc double(x) { return x + x }" 42
ret "proc main() { return add(mul(2, 3), 4) } proc add(a, b) { return a + b } proc mul(a, b) { return a * b }" 10
ret "proc main() { return fact(5) } proc fact(n) { state r { to b when (n < 2) return n * fact(n - 1) } state b { return 1 } }" 120
ret "proc main() { return fib(10) } proc fib(n) { state r { to b when (n < 2) return fib(n - 1) + fib(n - 2) } state b { return n } }" 55
ret "proc main() { return sumto(10) } proc sumto(n) { let t = 0 let i = 1 state l { to b when (i <= n) return t } state b { t = t + i i = i + 1 to l } }" 55
# slice 4 — byte[]/word[] memory
ret "proc main() { let b = 2097152 byte[b] = 65 byte[b + 1] = 66 return byte[b] + byte[b + 1] }" 131
ret "proc main() { let base = 2097152 let i = 0 state fill { to fb when (i < 5) to si } state fb { word[base + i * 8] = i * i i = i + 1 to fill } state si { let t = 0 i = 0 to sl } state sl { to sb when (i < 5) return t } state sb { t = t + word[base + i * 8] i = i + 1 to sl } }" 30
# slice 5 — char literals (intrinsics/emit/IO are covered end-to-end by selfhost.sh)
ret "proc main() { return 'A' }" 65
ret "proc main() { return '0' + 9 }" 57
ret "proc main() { let c = 'Z' state s { to y when (c == 90) return 0 } state y { return 42 } }" 42
ret "proc main() { return f(10, 20, 30) } proc f(a, b, c) { return a + c }" 40
ret "proc main() { return g(1, 2, 3, 4) } proc g(a, b, c, d) { return a + b + c + d }" 10
echo "bc.beta (slices 1-6, per-feature): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
