#!/usr/bin/env sh
# TRANSLATION VALIDATION, slice 0 — each compilation certified against the source's meaning (D3).
#
# The kernel diamond checks native-vs-meaning agreement by SHELL COMPARISON (test-based). This gate
# upgrades the check to a PROOF for straight-line +/* programs: the program is compiled natively and
# RUN; its actual exit E is encoded (tv-encode.py, untrusted) into a delta claim
#     (= <the program's meaning as a p/m arithmetic term> <unary E>)  (refl <unary E>)
# and check.beta must ACCEPT — the trust anchor's own conversion rule RE-COMPUTES the meaning, so
# acceptance certifies "this compilation produced the source's meaning" inside the kernel, not in a
# shell. A MISCOMPILATION (simulated by validating against E±1) yields a claim conversion cannot
# reach, and delta REJECTS it — a wrong compilation cannot be validated.
#
# Trust boundary (stated, per the honest-edges discipline): the encoder is untrusted; a bad encoding
# either fails outright or mis-states the meaning, and meaning-fidelity is independently pinned by the
# kernel diamond over the same translator output. Slice 0 scope: whole-program-result-level validation
# of straight-line +/* programs; loops (via delta's recursive user functions), subtraction, and
# instruction-level refinement are later slices.
#
# Native leg needs macOS arm64 + cargo/clang; skips cleanly otherwise.
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "translation-validation SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign python3; do command -v "$t" >/dev/null 2>&1 || { echo "translation-validation SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "translation-validation FAIL — bc build"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b omega2gamma.beta     "$T/o2g.exe"   || { echo "translation-validation FAIL — build omega2gamma.beta"; exit 1; }
b ../delta/check.beta  "$T/check.exe" || { echo "translation-validation FAIL — build check.beta"; exit 1; }
( cd ../epsilon-rs && cargo build -q 2>/dev/null ) || { echo "translation-validation FAIL — cargo build"; exit 1; }
BE=../epsilon-rs/target/debug/beta

PASS=0; FAIL=0
# tv DESC BODY : compile BODY natively, run it, and have delta certify that the observed exit is the
# source's meaning. Then require the MISCOMPILE simulation (exit+1) to be REJECTED.
tv() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  EPS_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  set +e; "$T/p"; nat=$?; set -e
  g=$("$T/o2g.exe" < "$T/p.alp" 2>/dev/null)
  cert=$(printf '%s\n' "$g" | python3 tv-encode.py "$nat") || { FAIL=$((FAIL+1)); echo "  FAIL $1 : encode (outside slice-0 subset?)"; return; }
  set +e
  v=$(printf '%s' "$cert" | "$T/check.exe")
  badcert=$(printf '%s\n' "$g" | python3 tv-encode.py $((nat + 1)))
  vb=$(printf '%s' "$badcert" | "$T/check.exe")
  set -e
  if [ "$v" = "accept" ] && [ "$vb" = "reject" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : validate=$v (want accept), miscompile-sim=$vb (want reject), exit=$nat"; fi
}

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

echo "translation validation slice 0 (delta certifies each compilation's result IS the source's meaning): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
