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
# dia DESC  BODY  EXPECT : BODY is the straight-line main body (lets + a final exit_process(EXPR)).
# native exit, Rust-free eps2gamma-route exit, the Rust gamma_emit route, and EXPECT must all agree.
dia() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  EPS_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$("$T/e2g.exe" < "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : eps2gamma emitted nothing"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; mine=$?; set -e
  rg=$(EPS_EMIT=gamma "$BE" "$T/p.alp" 2>/dev/null); set +e; printf '%s\n' "$rg" | "$T/interp.exe" >/dev/null; rgi=$?; set -e
  if [ "$nat" = "$mine" ] && [ "$nat" = "$3" ] && [ "$nat" = "$rgi" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat eps2gamma=$mine rustgamma=$rgi expect=$3"; fi
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

echo "eps2gamma diamond (native == Rust-free eps2gamma->interp == Rust gamma_emit): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
