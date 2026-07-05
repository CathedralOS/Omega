#!/usr/bin/env sh
# PRODREC SEAM — the product eliminator's trust-anchor rule, cross-checked against the independent reference.
#
# `prodrec` (product elimination: from a motive P holding on a single-constructor type's SOLE constructor,
# conclude ∀x. P(x)) now lives in BOTH the trusted kernel (check.beta) and the independent reference checker
# (check_ref.py), each with the SAME soundness guard: prodrec fires only on a cid explicitly declared a product
# via `(prod cid)`. This gate is the D4 seam-per-capability check for that rule — it confirms check.beta and
# check_ref DECIDE prodrec certs IDENTICALLY across the accepting case and every guard/soundness reject control:
#   accept   (prod 70) declared + prodrec on 70 proving the trivial motive P(x):=x=x     -> both accept
#   guard    the SAME cert with (prod 70) removed                                          -> both reject
#   sum-cid  prodrec aimed at a 2-constructor (sum) cid 61, not declared a product         -> both reject
#   badcase  the case proves the WRONG proposition (con_case mismatch)                     -> both reject
# A checker that drifted on the guard (e.g. dropped it, licensing an unsound ∀ over a sum type) would break the
# agreement here. checker.gamma does not yet have prodrec (step 3 of the climb), so the 3-checker diamond is not
# yet closed on prodrec certs and no corpus proof emits prodrec — this 2-checker seam is the current guarantee.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "prodrec-seam: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "prodrec-seam FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "prodrec-seam FAIL — build check.beta"; exit 1; }

GOAL='(All (= (v 0) (v 0)))'
GOODCASE='(gen (gen (refl (k 70 (v 1) (v 0)))))'
ACCEPT="(data 70 2 0 0)(prod 70)$GOAL(prodrec 70 (= (v 0) (v 0)) $GOODCASE)"
NOGUARD="(data 70 2 0 0)$GOAL(prodrec 70 (= (v 0) (v 0)) $GOODCASE)"
SUMCID="(data 70 2 0 0)(data 61 2 0 1)(prod 70)$GOAL(prodrec 61 (= (v 0) (v 0)) (gen (gen (refl (k 61 (v 1) (v 0))))))"
BADCASE="(data 70 2 0 0)(prod 70)$GOAL(prodrec 70 (= (v 0) (v 0)) (gen (gen (refl (k 70 (v 0) (v 0))))))"

ok=1
seam() {  # LABEL CERT EXPECTED
  vb=$(printf '%s' "$2" | perl -e 'alarm 30; exec @ARGV' "$T/check.exe" 2>/dev/null)
  vr=$(printf '%s' "$2" | python3 check_ref.py 2>/dev/null)
  agree=no; [ "$vb" = "$vr" ] && agree=yes
  echo "  $1: check.beta=$vb check_ref=$vr agree=$agree (expected $3)"
  { [ "$vb" = "$3" ] && [ "$vr" = "$3" ]; } || ok=0
}
seam "accept  (product declared)      " "$ACCEPT"  accept
seam "guard   ((prod 70) removed)     " "$NOGUARD" reject
seam "sum-cid (prodrec on sum ctor 61)" "$SUMCID"  reject
seam "badcase (case proves wrong prop)" "$BADCASE" reject

echo "prodrec seam (check.beta and check_ref decide the product eliminator identically; guard + soundness controls rejected by both): $([ "$ok" = 1 ] && echo PASS || echo FAIL)"
[ "$ok" = 1 ]
