#!/usr/bin/env sh
# TRUST-ANCHOR DIAMOND, independent point — an auditable reference checker (check_ref.py) agrees with
# check.beta on first-order logic (propositional + All/Exists).
#
# The checker is the trust anchor: it decides which proofs are valid. Its two implementations — check.beta
# (in Beta) and checker.gamma (in Gamma) — are diamonded against each other, but BOTH are lattice-lineage
# (compiled by bc). check_ref.py is a third, INDEPENDENT realization in Python of the checker's logic core: intuitionistic
# propositional natural deduction (->, &, +, bot intro+elim) AND the first-order rules (All/Exists with de
# Bruijn: gen/inst/wit/unpack, capture-avoiding), short enough to read against the rules. This gate fuzzes it
# against check.beta on random first-order proofs, requiring identical accept/reject (both accept a proof
# against its true goal; both reject it against a perturbed, wrong-type goal). So the LOGICAL soundness of the
# trust anchor is pinned by an independent, auditable implementation — the last rung to get one. UNTRUSTED and
# checked; the runtime never runs it. (Equality-conversion / induction remain check.beta-only, later slices.)
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "check-ref diamond: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "check-ref diamond: bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "check-ref diamond: check.beta build failed"; exit 1; }

if python3 check-ref-fuzz.py "$T/check.exe" "${1:-200}" > "$T/out" 2>&1; then
  echo "trust-anchor diamond (independent check_ref.py agrees with check.beta on first-order logic proofs): $(cat "$T/out")"
else
  echo "trust-anchor diamond FAILED:"; cat "$T/out"; exit 1
fi
