#!/usr/bin/env sh
# PROOF-LIBRARY CROSS-CHECK — the diversity thesis applied to the WHOLE theorem library.
#
# elab-test.sh already checks that every proofs/*.elab elaborates to a certificate the trusted check.beta
# accepts. But that is a SINGLE checker. The lattice's diversity thesis says every certificate should be
# decided identically by an INDEPENDENT checker; the trust-anchor diamond establishes check_ref.py ==
# check.beta on a rule-coverage FUZZ corpus, but the real compositional theorems (the FTA, sqrt2
# irrationality, the list/number-theory library — 200+ proofs) were only ever run through check.beta.
#
# This gate re-runs the ENTIRE library through check_ref.py — the independent, auditable Python reference
# checker — and requires ACCEPT-and-AGREE on every proof. A divergence would expose a bug in a checker OR an
# elaborator cert that exploits a check.beta-specific quirk. NEGATIVE CONTROLS (a goal-perturbed proof and
# hand-crafted false claims) must be REJECTED by BOTH, so the agreement is discriminating, not vacuous.
# (checker.gamma covers only ~8 of these forms — integers/predicates/etc. are beyond its current surface — and
# those are already triple-checked by checker-diamond.sh; this gate is the check.beta vs check_ref leg for ALL.)
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "proofs-crosscheck: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "proofs-crosscheck: bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < check.beta > "$T/p.asm" 2>/dev/null && "$ASM" < "$T/p.asm" > "$T/p.tape" 2>/dev/null \
  && stamp_seed "$T/p.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "proofs-crosscheck: check.beta build failed"; exit 1; }
CHECK="$T/check.exe"

PASS=0; FAIL=0
for f in proofs/*.elab; do
  cert=$(python3 elab.py < "$f" 2>/dev/null)
  if [ -z "$cert" ]; then FAIL=$((FAIL+1)); echo "  FAIL $f : elaboration errored"; continue; fi
  vb=$(printf '%s' "$cert" | "$CHECK" 2>/dev/null)
  vr=$(printf '%s' "$cert" | python3 check_ref.py 2>/dev/null)
  if [ "$vb" = accept ] && [ "$vr" = accept ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $(basename "$f") : check.beta=$vb check_ref=$vr (must both accept)"; fi
done

# NEGATIVE CONTROLS — both checkers must REJECT. (1) a goal-perturbed real proof (a+0=a becomes a+0=s a);
# (2)/(3) hand-crafted false claims. If either checker accepted any, the agreement above would be vacuous.
NEG=0; NEGOK=0
ncheck() {  # $1 = cert text ; both check.beta and check_ref must reject
  NEG=$((NEG+1))
  vb=$(printf '%s' "$1" | "$CHECK" 2>/dev/null); vr=$(printf '%s' "$1" | python3 check_ref.py 2>/dev/null)
  if [ "$vb" != accept ] && [ "$vr" != accept ]; then NEGOK=$((NEGOK+1))
  else FAIL=$((FAIL+1)); echo "  FAIL negative-control : check.beta=$vb check_ref=$vr (both must reject)"; fi
}
badcert=$(sed 's/(= (+ x1 z) x1)/(= (+ x1 z) (s x1))/' proofs/add-zero-right.elab | python3 elab.py 2>/dev/null)
ncheck "$badcert"
ncheck '(= (s z) z) (refl (s z))'                                   # 1 = 0
ncheck '(All (= (v 0) (s (v 0)))) (gen (refl (v 0)))'               # a = s a

echo "proof-library cross-check (every proofs/*.elab accepted by check.beta AND the independent check_ref.py; perturbations rejected by both): $PASS proofs cross-checked, $NEGOK/$NEG negative controls rejected"
[ "$FAIL" = 0 ] && [ "$PASS" -gt 0 ] && [ "$NEGOK" = "$NEG" ]
