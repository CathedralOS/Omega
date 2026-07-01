#!/usr/bin/env sh
# RUST-FREE REFERENCE-ROUTE CONVERGENCE — the proof-carrying loop with NO Rust anywhere.
#
# epsilon-rs/convergence-reference.sh runs a certifier down the MEANING route, but its epsilon->gamma
# translator is `EPS_EMIT=gamma` (Rust gamma_emit.rs). This is the same loop with that last Rust step
# removed: `epsilon/eps2gamma.beta` (Rust-free, alpha->beta->bc) translates the certifier to gamma,
# `gamma/interp.beta` (Rust-free) EXECUTES it and emits the certificate, and `delta/check.beta` (Rust-free)
# accepts it. Every artifact in the loop is in the hand-audited alpha-rooted lineage — so a real epsilon
# program's MEANING is computed, and its emitted proof checked, with zero Rust in the chain.
#
#   certify-add '2 3'  --(eps2gamma.beta)-->  gamma  --(interp.beta)-->  (= (p (s (s z)) ...) ...) (refl ...)
#                                                                        --(check.beta)-->  accept
#
# Needs no cargo/clang/codesign (the Rust-free route never compiles native code) — only bc, so it runs
# anywhere the rest of the lattice builds. A WRONG certificate is REJECTED here too (the negative control),
# so the loop is meaningful, not vacuous. Small-number certifiers only: a large certificate's unary
# numerals exhaust interp's arena.
# No `set -e`: interp/check exit with the alpha VM's result byte (captured inside `v=$(…)`); judge by stdout.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "convergence-reference(rust-free) FAIL — bc build"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b eps2gamma.beta        "$T/e2g.exe"    || { echo "convergence-reference(rust-free) FAIL — build eps2gamma.beta"; exit 1; }
b ../gamma/interp.beta  "$T/interp.exe" || { echo "convergence-reference(rust-free) FAIL — build interp.beta"; exit 1; }
b ../delta/check.beta   "$T/check.exe"  || { echo "convergence-reference(rust-free) FAIL — build check.beta"; exit 1; }

PASS=0; FAIL=0
# ref SAMPLE "in-bytes" EXPECT : eps2gamma translates the certifier; interp.beta runs it on that stdin
# (baked as a gamma list into the STDIN placeholder); decode the emitted certificate from interp's output
# list; require check.beta's verdict to be EXPECT.
ref() {
  rev=""; for x in $2; do rev="$x $rev"; done
  list="Nil"; for x in $rev; do list="(Cons $x $list)"; done
  g=$("$T/e2g.exe" < "../epsilon-rs/samples/$1.alp" 2>/dev/null | sed "s/STDIN/$list/")
  [ -n "$g" ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted"; return; }
  cert=$(printf '%s\n' "$g" | "$T/interp.exe" 2>/dev/null | grep -oE '[0-9]+' | awk '{printf "%c",$1}')
  v=$(printf '%s' "$cert" | "$T/check.exe")
  if [ "$v" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta returned [$v], expected $3 (cert: $cert)"; fi
}

# diverse cert kinds, each COMPUTED by the Rust-free reference interpreter and ACCEPTED by the trust anchor:
ref certify-add     "50 32 51"             accept   # '2 3'     -> equality / refl (a+b)
ref certify-product "50 32 51 32 53"       accept   # '2 3 5'   -> inductive predicate ProdIs (list product)
ref certify-member  "53 32 53 32 54 32 55" accept   # '5 5 6 7' -> inductive predicate Mem (list membership)
ref certify-divides "51 32 49 50"          accept   # '3 12'    -> existential (3 divides 12)
ref certify-wrong   "50 32 51"             reject   # NEGATIVE CONTROL: the buggy 'a+b = a+b+1' cert is rejected

echo "reference-route convergence, RUST-FREE (eps2gamma.beta -> interp.beta; cert checked by check.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
