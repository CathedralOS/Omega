#!/usr/bin/env sh
# RUST-FREE REFERENCE-ROUTE CONVERGENCE — the proof-carrying loop with NO Rust anywhere.
#
# delta-rs/convergence-reference.sh runs a certifier down the MEANING route, but its delta->gamma
# translator is `DELTA_EMIT=gamma` (Rust gamma_emit.rs). This is the same loop with that last Rust step
# removed: `omega2gamma.beta` (Rust-free, alpha->beta->bc) translates the certifier to gamma,
# `gamma/interp.beta` (Rust-free) EXECUTES it and emits the certificate, and `proof-kernel/check.beta` (Rust-free)
# accepts it. Every artifact in the loop is in the hand-audited alpha-rooted lineage — so a real delta
# program's MEANING is computed, and its emitted proof checked, with zero Rust in the chain.
#
#   certify-add '2 3'  --(omega2gamma.beta)-->  gamma  --(interp.beta)-->  (= (p (s (s z)) ...) ...) (refl ...)
#                                                                        --(check.beta)-->  accept
#
# Covers diverse cert kinds AND, crucially, the OMEGA SAFETY OBLIGATIONS a verifying compiler emits — array
# bounds (i*n+j < m*n), access conjunctions, and division-by-zero — the exact VC shapes the summit rung
# produces, here COMPUTED and CHECKED with zero Rust. Each has a mutated-cert negative control that must be
# rejected, so acceptance is meaningful, not vacuous.
#
# Needs no cargo/clang/codesign (the Rust-free route never compiles native code) — only bc, so it runs
# anywhere the rest of the lattice builds. Small-number certifiers only: a large certificate's unary
# numerals exhaust interp's arena.
# No `set -e`: interp/check exit with the alpha VM's result byte (captured inside `v=$(…)`); judge by stdout.
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
( cd "${OMEGA_PATH_BETA_COMPILER_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "convergence-reference(rust-free) FAIL — bc build"; exit 1; }
b() { "${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b "${OMEGA_PATH_OMEGA0}/meaning/omega2gamma.beta" "$T/omega2gamma.exe" \
  || { echo "convergence-reference(rust-free) FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta  "$T/interp.exe" || { echo "convergence-reference(rust-free) FAIL — build interp.beta"; exit 1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta   "$T/check.exe"  || { echo "convergence-reference(rust-free) FAIL — build check.beta"; exit 1; }

PASS=0; FAIL=0
# _emit SAMPLE "ascii-stdin" : omega2gamma translates the certifier; interp.beta runs it with that stdin
# (the ASCII is turned into the gamma byte list baked into the STDIN placeholder); prints the emitted cert.
_emit() {
  bytes=$(printf '%s' "$2" | od -An -tu1 | tr ' ' '\n' | grep -vE '^$')
  rev=""; for x in $bytes; do rev="$x $rev"; done
  list="Nil"; for x in $rev; do list="(Cons $x $list)"; done
  "$T/omega2gamma.exe" < "${OMEGA_PATH_DELTA_RUST}/samples/$1.alp" 2>/dev/null | sed "s/STDIN/$list/" | "$T/interp.exe" 2>/dev/null \
    | grep -oE '[0-9]+' | awk '{printf "%c",$1}'
}
# ref SAMPLE "ascii" EXPECT : the cert check.beta emits must be EXPECT.
ref() {
  cert=$(_emit "$1" "$2")
  [ -n "$cert" ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : no certificate emitted"; return; }
  v=$(printf '%s' "$cert" | "$T/check.exe")
  if [ "$v" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta returned [$v], expected $3"; fi
}
# refbad SAMPLE "ascii" SED : a MUTATED (false) certificate must be REJECTED — the loop is not vacuous.
refbad() {
  cert=$(_emit "$1" "$2" | sed "$3")
  v=$(printf '%s' "$cert" | "$T/check.exe")
  if [ "$v" = "reject" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 (mutated) : expected reject, got [$v]"; fi
}
# refnot SAMPLE "ascii" : the program's output must NOT be an accepted proof (e.g. it refused with a
# diagnostic instead of a certificate — the checker must not accept whatever came out).
refnot() {
  v=$(_emit "$1" "$2" | "$T/check.exe")
  if [ "$v" = "accept" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 (unsafe input) : checker ACCEPTED [$v]"; else
    PASS=$((PASS+1)); fi
}

# diverse cert kinds, each COMPUTED by the Rust-free reference interpreter and ACCEPTED by the trust anchor:
ref certify-add     "2 3"      accept   # equality / refl (a+b)
ref certify-product "2 3 5"    accept   # inductive predicate ProdIs (list product)
ref certify-member  "5 5 6 7"  accept   # inductive predicate Mem (list membership)
ref certify-divides "3 12"     accept   # existential (3 divides 12)
ref certify-wrong   "2 3"      reject   # NEGATIVE CONTROL: the buggy 'a+b = a+b+1' cert is rejected

# OMEGA SAFETY OBLIGATIONS — the VC shapes a verifying compiler must discharge (array bounds, division),
# here computed AND checked with zero Rust. Each mutated variant is rejected, so acceptance is meaningful.
ref    certify-lt       "2 5"              accept   # a < b (ordering; the shape every bounds VC reduces to)
ref    certify-bounds   "2 5 3 4"          accept   # a 2D array-bounds obligation  i*n+j < m*n
ref    certify-accesses "2 5 3 4  1 3 0 2" accept   # a CONJUNCTION: every access in a sequence is in bounds
ref    certify-safety   "b 2 5 3 4  d 7"   accept   # MIXED: array-bounds AND division-by-zero in one proof
refbad certify-lt       "2 5"              's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g'
refbad certify-safety   "b 2 5 3 4  d 7"   's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/'

# THE CERTIFYING COMPILER FRONTEND — source in, safety proof out, zero Rust. certify-source reads a tiny
# SOURCE language (arr/get/div/band), recognises keywords (value-returning self-methods), resolves each
# access against the declared array, GENERATES the VCs, and emits one conjunction certificate. Unsafe
# source gets no proof (it prints an "unsafe" diagnostic, which the checker naturally rejects).
ref    certify-source "arr 4 5  get 2 3  div 7  get 1 0" accept
refnot certify-source "arr 2 3  get 5 5"                          # out-of-bounds source -> no accepted proof

echo "reference-route convergence, RUST-FREE (omega2gamma.beta -> interp.beta; cert checked by check.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
