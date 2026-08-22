#!/usr/bin/env sh
# TRANSLATION VALIDATION — each compilation certified against the source's meaning (D3).
#
# The kernel diamond checks native-vs-meaning agreement by SHELL COMPARISON (test-based). This gate
# upgrades the check to a PROOF for straight-line + - * < == programs: the program is compiled natively and
# RUN; its actual exit E is encoded (tv-encode.py, untrusted) into a delta claim
#     (= <the program's meaning as a p/m arithmetic term> <unary E>)  (refl <unary E>)
# and check.beta must ACCEPT — the trust anchor's own conversion rule RE-COMPUTES the meaning, so
# acceptance certifies "this compilation produced the source's meaning" inside the kernel, not in a
# shell. A MISCOMPILATION (simulated by validating against E±1) yields a claim conversion cannot
# reach, and the proof kernel REJECTS it — a wrong compilation cannot be validated.
#
# Trust boundary (stated, per the honest-edges discipline): the encoder is untrusted; a bad encoding
# either fails outright or mis-states the meaning, and meaning-fidelity is independently pinned by the
# kernel diamond over the same translator output. Scope: whole-program-result-level validation of
# straight-line + - * < == / % programs (comparisons decide 0/1 and div/mod re-subtract, all inside the
# kernel) AND bounded state-machine loops (encoded as a delta fuel-fold whose guard + body the kernel
# re-evaluates each iteration, N loop-carried locals packed into user Pairs; the encoder abstract-executes
# the loop to get a safe trip bound). Data-dependent loops whose body uses div/mod (e.g. gcd) are instead
# UNROLLED — the encoder knows the trip count, so it steps the loop symbolically and each mod becomes a
# standalone kernel-recomputed op; deep unrollings bail on the arena wall.
#
# Native leg needs macOS arm64 + cargo/clang; skips cleanly otherwise.
set -e
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
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "translation-validation SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign python3; do command -v "$t" >/dev/null 2>&1 || { echo "translation-validation SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "translation-validation FAIL — bc build"; exit 1; }
b() { "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b omega2gamma.beta     "$T/o2g.exe"   || { echo "translation-validation FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/check.beta  "$T/check.exe" || { echo "translation-validation FAIL — build check.beta"; exit 1; }
( cd "${OMEGA_PATH_DELTA_RUST}" && cargo build -q 2>/dev/null ) || { echo "translation-validation FAIL — cargo build"; exit 1; }
BE="${OMEGA_PATH_DELTA_RUST}"/target/debug/delta

PASS=0; FAIL=0
# tvcore DESC : the .alp program is already at $T/p.alp — compile natively, run it, and have delta certify
# that the observed exit IS the source's meaning; then require the MISCOMPILE simulation (exit+1) REJECTED.
tvcore() {
  DELTA_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  set +e; "$T/p"; nat=$?; set -e
  g=$("$T/o2g.exe" < "$T/p.alp" 2>/dev/null)
  cert=$(printf '%s\n' "$g" | python3 tv-encode.py "$nat") || { FAIL=$((FAIL+1)); echo "  FAIL $1 : encode (outside subset?)"; return; }
  set +e
  v=$(printf '%s' "$cert" | "$T/check.exe")
  badcert=$(printf '%s\n' "$g" | python3 tv-encode.py $((nat + 1)))
  vb=$(printf '%s' "$badcert" | "$T/check.exe")
  set -e
  if [ "$v" = "accept" ] && [ "$vb" = "reject" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : validate=$v (want accept), miscompile-sim=$vb (want reject), exit=$nat"; fi
}
# tv DESC BODY : $2 is the Main::main body (wrapped in the standard preamble).
tv() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  tvcore "$1"
}
# tvm DESC PROGRAM : $2 is a FULL program (free machines + Main::main), for cross-machine cases.
tvm() { printf '%s\n' "$2" > "$T/p.alp"; tvcore "$1"; }

tv "literal"          '    self.console.exit_process(42)'
tv "product"          '    let a: i32 = 6 * 7;
    self.console.exit_process(a)'
tv "sum-of-products"  '    let a: i32 = 3 + 4;
    let b: i32 = a * 5;
    self.console.exit_process(b + 7)'
tv "precedence"       '    self.console.exit_process(2 + 8 * 5)'
tv "local chain"      '    let a: i32 = 2 * 3;
    let b: i32 = a + 4;
    let c: i32 = b * 4;
    self.console.exit_process(c + 2)'
tv "subtraction"      '    let a: i32 = 50 - 8;
    self.console.exit_process(a)'
tv "mixed +-*"        '    let a: i32 = 9 - 2;
    let b: i32 = a * 6;
    self.console.exit_process(b - 0)'
tv "nested minus"     '    self.console.exit_process((10 - 3) * (8 - 2))'
tv "less-than true"   '    let c: i32 = 3 < 5;
    self.console.exit_process(c)'
tv "less-than false"  '    let c: i32 = 5 < 3;
    self.console.exit_process(c)'
tv "equal true"       '    let c: i32 = 7 == 7;
    self.console.exit_process(c)'
tv "not-equal"        '    let c: i32 = 7 != 4;
    self.console.exit_process(c)'
tv "leq boundary"     '    let c: i32 = 4 <= 4;
    self.console.exit_process(c)'
tv "geq false"        '    let c: i32 = 6 >= 9;
    self.console.exit_process(c)'
tv "branch predicate" '    let a: i32 = 8;
    self.console.exit_process((a > 5) * 30 + (a < 5) * 7 + 12)'
tv "compare then sum" '    let a: i32 = 4 == 4;
    let b: i32 = 3 < 9;
    self.console.exit_process(a * 20 + b * 22)'
# bounded state-machine loops: delta re-runs the guard + body each iteration (fuel-fold, Pair-packed
# loop-carried locals). exit is the source's meaning, exit+1 is unreachable by the re-evaluation.
tv "loop sum 1..4"    '    let i: i32 = 0;
    let s: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition i < 4 { true -> bd()  false -> dn() } }
    state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s) }'
tv "factorial 5!"     '    let i: i32 = 1;
    let a: i32 = 1;
    transition 0 { _ -> lp() }
    state lp() { transition i <= 5 { true -> bd()  false -> dn() } }
    state bd() { a = a * i; i = i + 1; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a) }'
tv "loop then offset" '    let i: i32 = 0;
    let s: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition i < 5 { true -> bd()  false -> dn() } }
    state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s + 3) }'
tv "countdown mul"    '    let n: i32 = 4;
    let a: i32 = 1;
    transition 0 { _ -> lp() }
    state lp() { transition 0 < n { true -> bd()  false -> dn() } }
    state bd() { a = a * 2; n = n - 1; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a) }'
tv "exit-when-true"   '    let n: i32 = 4;
    let a: i32 = 1;
    transition 0 { _ -> lp() }
    state lp() { transition n == 0 { true -> dn()  false -> bd() } }
    state bd() { a = a * 2; n = n - 1; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a) }'
tv "3 carried locals" '    let i: i32 = 0;
    let s: i32 = 0;
    let p: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition i < 4 { true -> bd()  false -> dn() } }
    state bd() { i = i + 1; s = s + i; p = p + 2; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s + p) }'
# div / mod: kernel re-subtracts (small quotients; larger ones the encoder bails on, arena wall).
tv "divide"           '    let q: i32 = 17 / 5;
    self.console.exit_process(q)'
tv "modulo"           '    let r: i32 = 17 % 5;
    self.console.exit_process(r)'
tv "div and mod"      '    let a: i32 = 23;
    self.console.exit_process((a / 4) * 10 + (a % 4))'
tv "mod under arith"  '    let n: i32 = 20;
    self.console.exit_process(n % 7 + n / 7 * 6)'
# data-dependent loops with div/mod in the body: UNROLLED (trip count is data-dependent but known to the
# encoder), each mod a standalone kernel-recomputed op. gcd (Euclid) is the flagship; deep ones bail.
tv "gcd(48,36)=12"    '    let a: i32 = 48;
    let b: i32 = 36;
    let t: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition b == 0 { true -> dn()  false -> st() } }
    state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a) }'
tv "gcd(100,60)=20"   '    let a: i32 = 100;
    let b: i32 = 60;
    let t: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition b == 0 { true -> dn()  false -> st() } }
    state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a) }'
tv "digit-sum-ish"    '    let n: i32 = 47;
    let s: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition n == 0 { true -> dn()  false -> st() } }
    state st() { s = s + n % 10; n = n / 10; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s) }'
# cross-machine calls: value-returning FREE machines. The encoder INLINES each call (bind params, encode
# the callee body) so delta recomputes the whole nested computation; the meaning route already handles them.
tvm "nested call" 'boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine add(a: i32, b: i32) -> i32 { return a + b; }
machine dbl(x: i32) -> i32 { return x + x; }
machine Main::main(&mut self) { self.console.exit_process(add(dbl(15), 12)) }'
tvm "call under arith" 'boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine sq(x: i32) -> i32 { return x * x; }
machine Main::main(&mut self) { self.console.exit_process(sq(6) + sq(2) + 2) }'
tvm "chained calls" 'boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine inc(x: i32) -> i32 { return x + 1; }
machine Main::main(&mut self) { self.console.exit_process(inc(inc(inc(39)))) }'

echo "translation validation (delta re-evaluates each compilation's result — straight-line + - * < == / %, bounded loops, AND cross-machine calls — and certifies it IS the source's meaning): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
