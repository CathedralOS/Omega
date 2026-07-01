#!/usr/bin/env sh
# EPS2GAMMA DIAMOND — epsilon's meaning, now with RUST OFF the meaning route.
#
# The epsilon-meaning diamond (epsilon-rs/epsilon-meaning-diamond.sh) already pins epsilon's meaning
# against native execution — but its translator, gamma_emit.rs, is RUST, so Rust still sat on the
# meaning side. This diamond removes it: epsilon/eps2gamma.beta is a Rust-FREE epsilon->gamma
# translator (built alpha->beta->bc, the same lineage as interp.beta). Each program is run TWO ways
# and the exit codes must agree:
#   (1) NATIVE     — compiled by the epsilon-rs aarch64 backend and executed (the reference)
#   (2) EPS2GAMMA  — eps2gamma.beta (Rust-free) translates it to gamma; interp.beta (Rust-free) runs it
# Both artifacts on route (2) are in the Rust-free trust lineage, so epsilon's meaning is now defined
# without Rust for the supported subset. As a bonus cross-check we also confirm the Rust-free route
# agrees with the existing Rust gamma_emit.rs route (EPS_EMIT=gamma) — the two translators converge.
#
# SLICE 0: straight-line integer `main` (lets + exit_process terminal; + - * / %, parens, locals).
# The subset grows exactly as eps2gamma.beta grows (comparisons, states, calls, ... — later slices).
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain (the native route needs them).
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "eps2gamma diamond SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "eps2gamma diamond SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# Rust-free lineage: alpha seed -> beta assembler -> bc -> {interp.exe, eps2gamma.exe}
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "eps2gamma diamond FAIL — bc build"; exit 1; }
BC=../beta-lang-rs/build/bc.exe
build_beta() { # src.beta  ->  out.exe   (bc -> assemble -> stamp)
  "$BC" < "$1" > "$T/b.asm" 2>/dev/null && "$ASM" < "$T/b.asm" > "$T/b.tape" 2>/dev/null \
    && stamp_seed "$T/b.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta ../gamma/interp.beta "$T/interp.exe" || { echo "eps2gamma diamond FAIL — build interp.beta"; exit 1; }
build_beta eps2gamma.beta        "$T/e2g.exe"    || { echo "eps2gamma diamond FAIL — build eps2gamma.beta"; exit 1; }

# native reference backend (Rust on-ramp — this is the thing being CHECKED, not trusted)
( cd ../epsilon-rs && cargo build -q 2>/dev/null ) || { echo "eps2gamma diamond FAIL — cargo build"; exit 1; }
BE=../epsilon-rs/target/debug/beta

PASS=0; FAIL=0
# _check DESC EXPECT : assumes $T/p.alp is written; native exit, Rust-free eps2gamma-route exit, the Rust
# gamma_emit route, and EXPECT must all agree.
_check() {
  EPS_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$("$T/e2g.exe" < "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : eps2gamma emitted nothing"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; mine=$?; set -e
  rg=$(EPS_EMIT=gamma "$BE" "$T/p.alp" 2>/dev/null); set +e; printf '%s\n' "$rg" | "$T/interp.exe" >/dev/null; rgi=$?; set -e
  if [ "$nat" = "$mine" ] && [ "$nat" = "$2" ] && [ "$nat" = "$rgi" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat eps2gamma=$mine rustgamma=$rgi expect=$2"; fi
}
# dia DESC BODY EXPECT : BODY is the main body; Main has no scalar fields.
dia() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diaf DESC BODY EXPECT : like dia but Main also has scalar i32 fields `i` and `s` (self data slice).
diaf() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; i: i32; s: i32; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diac DESC MACHINES EXPECT : MACHINES is the full machine section (free machines + Main::main) — for
# the cross-machine-call slice, where the body needs sibling `machine name(..) -> i32 { .. }` definitions.
diac() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\n%s\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}

dia "literal"            '    self.console.exit_process(42)' 42
dia "add"                '    self.console.exit_process(40 + 2)' 42
dia "sub"                '    self.console.exit_process(50 - 8)' 42
dia "mul"                '    self.console.exit_process(6 * 7)' 42
dia "div"                '    self.console.exit_process(84 / 2)' 42
dia "mod"                '    self.console.exit_process(142 % 100)' 42
dia "precedence"         '    self.console.exit_process(2 + 8 * 5)' 42
dia "parens"             '    self.console.exit_process((2 + 4) * 7)' 42
dia "left-assoc sub"     '    self.console.exit_process(50 - 3 - 5)' 42
dia "one local"          '    let a: i32 = 6 * 7;
    self.console.exit_process(a)' 42
dia "local chain"        '    let a: i32 = 6 * 7;
    let b: i32 = a - 2;
    let c: i32 = (a + b) / 2;
    self.console.exit_process(a + b + c - 81)' 42
dia "local in arith"     '    let x: i32 = 10;
    let y: i32 = x * x;
    self.console.exit_process(y - 58)' 42

# slice 1 — comparisons (faithfully from interp's only two primitives eq/lt).
dia "lt true"            '    let c: i32 = 3 < 5;
    self.console.exit_process(c + 41)' 42
dia "gt false"           '    let c: i32 = 3 > 5;
    self.console.exit_process(c + 42)' 42
dia "eq true"            '    let c: i32 = 7 == 7;
    self.console.exit_process(c * 42)' 42
dia "ne / eq combo"      '    let a: i32 = 4 == 4;
    let b: i32 = 4 != 4;
    self.console.exit_process(a * 42 + b)' 42
dia "le boundary"        '    let a: i32 = 5 <= 5;
    let b: i32 = 6 <= 5;
    self.console.exit_process(a * 42 + b)' 42
dia "ge boundary"        '    let a: i32 = 5 >= 5;
    let b: i32 = 4 >= 5;
    self.console.exit_process(a * 42 + b)' 42
dia "cmp under arith"    '    let a: i32 = 10;
    let b: i32 = (a > 5) * 30 + (a < 5) * 7 + 12;
    self.console.exit_process(b)' 42

# slice 2 — state machines (mutually-recursive gamma defs, SSA-threaded locals, guarded transitions).
dia "loop sum 1..4"      '    let i: i32 = 0;
    let s: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition i < 4 { true -> bd()  false -> dn() } }
    state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s + 32); }' 42
dia "factorial-ish"      '    let i: i32 = 1;
    let a: i32 = 1;
    transition 0 { _ -> lp() }
    state lp() { transition i <= 5 { true -> bd()  false -> dn() } }
    state bd() { a = a * i; i = i + 1; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a - 78); }' 42
dia "gcd then offset"    '    let a: i32 = 90;
    let b: i32 = 48;
    let t: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition b == 0 { true -> dn()  false -> st() } }
    state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a + 36); }' 42
dia "int-pattern switch" '    let x: i32 = 2;
    let r: i32 = 0;
    transition 0 { _ -> pick() }
    state pick() { transition x { 0 -> za()  1 -> ob()  _ -> tw() } }
    state za() { r = 1; transition 0 { _ -> dn() } }
    state ob() { r = 7; transition 0 { _ -> dn() } }
    state tw() { r = 42; transition 0 { _ -> dn() } }
    state dn() { self.console.exit_process(r); }' 42

# slice 3 — self data fields (threaded g{i} slots alongside locals, zero-initialised).
diaf "field loop sum"    '    transition 0 { _ -> lp() }
    state lp() { transition self.i < 5 { true -> bd()  false -> dn() } }
    state bd() { self.i = self.i + 1; self.s = self.s + self.i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(self.s + 27); }' 42
diaf "field+local mix"   '    let k: i32 = 3;
    transition 0 { _ -> lp() }
    state lp() { transition self.i < k { true -> bd()  false -> dn() } }
    state bd() { self.i = self.i + 1; self.s = self.s + self.i * k; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(self.s + 24); }' 42
diaf "field cmp + arith" '    transition 0 { _ -> setup() }
    state setup() { self.i = 20; self.s = 22; transition 0 { _ -> dn() } }
    state dn() { self.console.exit_process(self.i + self.s); }' 42

# slice 4 — cross-machine calls (each reachable machine its own m{idx}_* defs; a call passes args + zeros).
diac "call chain (nested)" 'machine addk(a: i32, b: i32) -> i32 { return a + b; }
machine dbl(x: i32) -> i32 { return x + x; }
machine Main::main(&mut self) { let r: i32 = dbl(addk(20, 22)); self.console.exit_process(r - 42); }' 42
diac "recursive factorial" 'machine fact(n: i32) -> i32 { transition n < 2 { true -> one()  false -> rec() } state one() { return 1; } state rec() { return n * fact(n - 1); } }
machine Main::main(&mut self) { self.console.exit_process(fact(5) - 78); }' 42
diac "call inside a loop" 'machine inc(x: i32) -> i32 { return x + 1; }
machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 5 { true -> bd()  false -> dn() } } state bd() { i = inc(i); s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s + 27); } }' 42
diac "two-arg helper twice" 'machine amax(a: i32, b: i32) -> i32 { transition a < b { true -> hb()  false -> ha() } state ha() { return a; } state hb() { return b; } }
machine Main::main(&mut self) { let x: i32 = amax(10, 40); let y: i32 = amax(x, 2); self.console.exit_process(y + 2); }' 42

echo "eps2gamma diamond (native == Rust-free eps2gamma->interp == Rust gamma_emit): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
