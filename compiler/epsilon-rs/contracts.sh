#!/usr/bin/env sh
# STATIC CONTRACT DISCHARGE — the verification-compiler step. Where convergence.sh has an
# epsilon PROGRAM emit a certificate about a concrete computation, this has the epsilon
# COMPILER emit a certificate that a declared `ensures` contract holds SYMBOLICALLY, for all
# inputs, straight from the signature. The trust anchor (the delta checker, built by the
# alpha->beta->bc pipeline) then validates each at BUILD time: a true postcondition is
# accepted, a false one REJECTED. The compiler proves the contract; it is not trusted to.
#
# Skips cleanly without the cargo toolchain or the seed pipeline.
set -e
cd "$(dirname "$0")"
command -v cargo   >/dev/null 2>&1 || { echo "contracts SKIP — no cargo"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "contracts SKIP — no python3 (needed for the lemma library)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# 1. build the delta checker (trust anchor), exactly as the lattice does
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "contracts FAIL — bc build"; exit 1; }
if ../beta-lang-rs/build/bc.exe < ../delta/check.beta > "$T/c.asm" 2>/dev/null \
   && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
   && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1; then :; else
  echo "contracts FAIL — could not build the delta checker"; exit 1; fi

# 2. build the epsilon compiler
cargo build -q 2>/dev/null || { echo "contracts FAIL — cargo build"; exit 1; }

# 3. generate the lemma library the compiler cites (add-zero-right etc.). It is prepended to
# every certificate; self-contained (refl/witness) certs simply ignore its defs. The proofs are SEARCHED by
# the prover (prover-contract-lib.py), not hand-written -- "automation discharges, the kernel checks": each
# library def is validated by check.beta along with the citation below.
python3 ../delta/prover-contract-lib.py > "$T/lib" 2>/dev/null \
  || { echo "contracts FAIL — building the lemma library"; exit 1; }
LIB=$(cat "$T/lib")

PASS=0; FAIL=0

# Every certificate the compiler discharges from samples/discharge.alp must be ACCEPTED (the
# lemma library is concatenated first). The loop reads from a file, not a pipe, so it runs in
# this shell and updates the tally.
EPS_EMIT=contracts ./target/debug/beta samples/discharge.alp > "$T/certs" \
  || { echo "contracts FAIL — emitting discharge.alp"; exit 1; }
[ -s "$T/certs" ] || { echo "contracts FAIL — no certificates emitted"; exit 1; }
while IFS= read -r cert; do
  [ -n "$cert" ] || continue
  v=$(printf '%s %s' "$LIB" "$cert" | "$T/check.exe" || true)
  if [ "$v" = accept ]; then PASS=$((PASS + 1)); echo "  ok   accept  $cert";
  else FAIL=$((FAIL + 1)); echo "  FAIL [$v]  $cert"; fi
done < "$T/certs"

# A FALSE contract must be REJECTED — the build-time catch is the whole point.
cat > "$T/false.alp" <<'EOF'
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine wrong(a: i32) -> i32 ensures result == a + a { return a; }
machine Main::main(&mut self) { let r: i32 = wrong(3); self.console.exit_process(r) }
EOF
FCERT=$(EPS_EMIT=contracts ./target/debug/beta "$T/false.alp")
fv=$(printf '%s %s' "$LIB" "$FCERT" | "$T/check.exe")
if [ "$fv" = reject ]; then PASS=$((PASS + 1)); echo "  ok   reject  (false contract caught)  $FCERT";
else FAIL=$((FAIL + 1)); echo "  FAIL false contract NOT rejected [$fv]  $FCERT"; fi

echo "static contract discharge (compiler proves ensures; trust anchor checks at build time): $PASS verified, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
