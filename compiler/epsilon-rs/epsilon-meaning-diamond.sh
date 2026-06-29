#!/usr/bin/env sh
# EPSILON-MEANING DIAMOND — the first thread of getting epsilon out of Rust.
#
# rungs/epsilon.md: epsilon's meaning is "Written in Delta/Gamma" -- defined by the reference
# interpreter, not the native (Rust on-ramp) backend. This diamond pins that meaning for the
# supported subset (straight-line integer code AND state machines): an epsilon program is run TWO ways and the exit codes must match:
#   (1) NATIVE   -- compiled by the epsilon-rs aarch64 backend and executed
#   (2) GAMMA    -- `EPS_EMIT=gamma` translates it to a gamma expression, which the Rust-FREE
#                   reference interpreter (interp.beta, built by the alpha->beta->bc pipeline) runs
# Agreement is evidence epsilon's native execution and its lattice-defined meaning coincide -- the
# same move that put gamma and the checker into the lineage, now reaching up to epsilon. As the
# supported subset grows (states, mutation, calls), this diamond widens with it.
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain (the native route needs them).
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "epsilon-meaning diamond SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "epsilon-meaning diamond SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# the gamma reference interpreter (trust-lineage: alpha seed -> beta asm -> bc -> interp.exe)
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "epsilon-meaning diamond FAIL — bc build"; exit 1; }
if ../beta-lang-rs/build/bc.exe < ../gamma/interp.beta > "$T/i.asm" 2>/dev/null \
   && "$ASM" < "$T/i.asm" > "$T/i.tape" 2>/dev/null \
   && stamp_seed "$T/i.tape" "$SEED" "$T/interp.exe" >/dev/null 2>&1; then :; else
  echo "epsilon-meaning diamond FAIL — could not build interp.beta"; exit 1; fi
cargo build -q 2>/dev/null || { echo "epsilon-meaning diamond FAIL — cargo build"; exit 1; }

PASS=0; FAIL=0
# dia DESC  SRC  EXPECT : native exit, gamma-interp exit, and EXPECT must all agree (exit codes are
# the low byte, so keep the result in 0..255).
dia() {
  printf '%s' "$2" > "$T/p.alp"
  EPS_ARCH=aarch64 ./target/debug/beta "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$(EPS_EMIT=gamma ./target/debug/beta "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted (outside the supported subset?)"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; gi=$?; set -e
  if [ "$nat" = "$gi" ] && [ "$nat" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat gamma=$gi expect=$3"; fi
}

H='boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; }'
dia "const"        "$H machine Main::main(&mut self) { self.console.exit_process(42); }" 42
dia "add"          "$H machine Main::main(&mut self) { let a: i32 = 2 + 3; self.console.exit_process(a); }" 5
dia "chain *,-"    "$H machine Main::main(&mut self) { let a: i32 = 2 + 3; let b: i32 = a * 4; let c: i32 = b - 1; self.console.exit_process(c); }" 19
dia "nested arith" "$H machine Main::main(&mut self) { let a: i32 = (2 + 3) * (4 + 1); self.console.exit_process(a); }" 25
dia "reuse local"  "$H machine Main::main(&mut self) { let a: i32 = 7; let b: i32 = a * a; let c: i32 = b - a; self.console.exit_process(c); }" 42
# division, modulo, and the full comparison set (faithfully encoded from lt/eq in gamma)
dia "div,mod"      "$H machine Main::main(&mut self) { let q: i32 = 17 / 5; let r: i32 = 17 % 5; self.console.exit_process(q * 10 + r); }" 32
dia "lt true"      "$H machine Main::main(&mut self) { let c: i32 = 3 < 5; self.console.exit_process(c); }" 1
dia "gt false"     "$H machine Main::main(&mut self) { let c: i32 = 3 > 5; self.console.exit_process(c); }" 0
dia "eq/ne"        "$H machine Main::main(&mut self) { let a: i32 = 4 == 4; let b: i32 = 4 != 4; self.console.exit_process(a * 2 + b); }" 2
dia "le boundary"  "$H machine Main::main(&mut self) { let a: i32 = 5 <= 5; let b: i32 = 6 <= 5; self.console.exit_process(a * 2 + b); }" 2
dia "ge boundary"  "$H machine Main::main(&mut self) { let a: i32 = 5 >= 5; let b: i32 = 4 >= 5; self.console.exit_process(a * 2 + b); }" 2
# STATE MACHINES — loops with mutation + guarded transitions, modeled as mutually-recursive gamma defs
dia "sum 1..4"     "$H machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 4 { true -> bd()  false -> dn() } } state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 10
dia "sum 1..10"    "$H machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 10 { true -> bd()  false -> dn() } } state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 55
dia "factorial 5"  "$H machine Main::main(&mut self) { let i: i32 = 1; let a: i32 = 1; transition 0 { _ -> lp() } state lp() { transition i <= 5 { true -> bd()  false -> dn() } } state bd() { a = a * i; i = i + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(a); } }" 120
dia "gcd 48,36"    "$H machine Main::main(&mut self) { let a: i32 = 48; let b: i32 = 36; let t: i32 = 0; transition 0 { _ -> lp() } state lp() { transition b == 0 { true -> dn()  false -> st() } } state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(a); } }" 12

echo "epsilon-meaning diamond (native execution vs gamma reference interpreter): $PASS agree, $FAIL disagree"
[ "$FAIL" = 0 ] || exit 1
