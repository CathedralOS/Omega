#!/usr/bin/env sh
# TRUST-ANCHOR DIAMOND, independent point — an auditable reference checker (implementations/reference/check_ref.py) agrees with
# implementations/beta/check.beta on logic (propositional + first-order), equality-conversion, and the TV cert language.
#
# The checker is the trust anchor: it decides which proofs are valid. Its two implementations — implementations/beta/check.beta
# (in Beta) and implementations/gamma/checker.gamma (in Gamma) — are diamonded against each other, but BOTH are lattice-lineage
# (compiled by bc). implementations/reference/check_ref.py is a third, INDEPENDENT realization in Python of the checker's logic core: intuitionistic
# propositional natural deduction (->, &, +, bot intro+elim) the first-order rules (All/Exists with de
# Bruijn: gen/inst/wit/unpack, capture-avoiding), AND equality by CONVERSION (refl + a Peano p/m normalizer,
# so `(= a b)` accepts iff a and b reduce to the same normal form), short enough to read against the rules.
# This gate fuzzes it against implementations/beta/check.beta on random proofs across all four categories — propositional,
# first-order, equality-conversion, and USER-FUNCTION arithmetic certificates (the actual TV cert language) — requiring identical accept/reject (both accept a proof
# against its true goal; both reject it against a perturbed, wrong-type goal). So the LOGIC + equality of the
# trust anchor are pinned by an independent, auditable implementation — the last rung to get one. UNTRUSTED and
# checked; the runtime never runs it. Curated induction (natind/listind/eqelim/disj/sinj), inductive-predicate
# (Mem/ProdIs/Perm), and named-lemma + generic-structural-induction (def/use/rec) corpora are cross-checked too
# — so check_ref independently realizes EVERY rule of the trust anchor: it is now a complete second checker.
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
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "check-ref diamond: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
"$T/bc.exe" < implementations/beta/check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "check-ref diamond: implementations/beta/check.beta build failed"; exit 1; }

if python3 corpus/fuzz/check-ref-fuzz.py "$T/check.exe" "${1:-200}" > "$T/out" 2>&1; then
  echo "trust-anchor diamond (independent implementations/reference/check_ref.py agrees with implementations/beta/check.beta on the COMPLETE rule set — logic, first-order, equality, induction, predicates, lemmas, TV certs): $(cat "$T/out")"
else
  echo "trust-anchor diamond FAILED:"; cat "$T/out"; exit 1
fi
