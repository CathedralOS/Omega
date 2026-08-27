#!/usr/bin/env sh
# PRODREC DIAMOND — the product eliminator's trust-anchor rule, cross-checked by all THREE checkers.
#
# `prodrec` (product elimination: from a motive P holding on a single-constructor type's SOLE constructor,
# conclude ∀x. P(x)) now lives in ALL THREE diamond checkers — the trusted kernel (implementations/beta/check.beta), the independent
# reference (implementations/reference/check_ref.py), and the gamma-language checker (implementations/gamma/checker.gamma via refcert_to_gamma) — each with the
# SAME soundness guard: prodrec fires only on a cid explicitly declared a product via `(prod cid)` (implementations/beta/check.beta
# and check_ref via a PRODUCTS table/set; implementations/gamma/checker.gamma via the Mkprod-vs-Mkspec spec constructor the translator
# picks). This gate is the D4 seam-per-capability check for that rule — it confirms all three DECIDE prodrec
# certs IDENTICALLY across the accepting case and every guard/soundness reject control:
#   accept   (prod 70) declared + prodrec on 70 proving the trivial motive P(x):=x=x     -> all three accept
#   guard    the SAME cert with (prod 70) removed                                          -> all three reject
#   sum-cid  prodrec aimed at a 2-constructor (sum) cid 61, not declared a product         -> all three reject
#   badcase  the case proves the WRONG proposition (con_case mismatch)                     -> all three reject
# A checker that drifted on the guard (e.g. dropped it, licensing an unsound ∀ over a sum type) breaks the
# agreement here. No corpus proof emits prodrec yet (that arrives with the ℤ signed-sum fold, step 4).
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "prodrec-seam: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/check.beta           "$T/check.exe"  || { echo "prodrec-seam FAIL — build implementations/beta/check.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "prodrec-seam FAIL — build interp.beta"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma)

GOAL='(All (= (v 0) (v 0)))'
GOODCASE='(gen (gen (refl (k 70 (v 1) (v 0)))))'
ACCEPT="(data 70 2 0 0)(prod 70)$GOAL(prodrec 70 (= (v 0) (v 0)) $GOODCASE)"
NOGUARD="(data 70 2 0 0)$GOAL(prodrec 70 (= (v 0) (v 0)) $GOODCASE)"
SUMCID="(data 70 2 0 0)(data 61 2 0 1)(prod 70)$GOAL(prodrec 61 (= (v 0) (v 0)) (gen (gen (refl (k 61 (v 1) (v 0))))))"
BADCASE="(data 70 2 0 0)(prod 70)$GOAL(prodrec 70 (= (v 0) (v 0)) (gen (gen (refl (k 70 (v 0) (v 0))))))"

ok=1
seam() {  # LABEL CERT EXPECTED
  vb=$(printf '%s' "$2" | perl -e 'alarm 30; exec @ARGV' "$T/check.exe" 2>/dev/null)
  vr=$(printf '%s' "$2" | python3 implementations/reference/check_ref.py 2>/dev/null)
  g=$(printf '%s' "$2" | python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/refcert_to_gamma.py 2>/dev/null)
  printf '%s\n%s\n' "$DEFS" "$g" | perl -e 'alarm 40; exec @ARGV' "$T/interp.exe" >/dev/null 2>&1
  r=$?; vg=undecided; [ "$r" = 1 ] && vg=accept; [ "$r" = 0 ] && vg=reject
  agree=no; { [ "$vb" = "$vr" ] && [ "$vr" = "$vg" ]; } && agree=yes
  echo "  $1: implementations/beta/check.beta=$vb check_ref=$vr implementations/gamma/checker.gamma=$vg agree=$agree (expected $3)"
  { [ "$vb" = "$3" ] && [ "$vr" = "$3" ] && [ "$vg" = "$3" ]; } || ok=0
}
seam "accept  (product declared)      " "$ACCEPT"  accept
seam "guard   ((prod 70) removed)     " "$NOGUARD" reject
seam "sum-cid (prodrec on sum ctor 61)" "$SUMCID"  reject
seam "badcase (case proves wrong prop)" "$BADCASE" reject

echo "prodrec diamond (implementations/beta/check.beta + check_ref + implementations/gamma/checker.gamma decide the product eliminator identically; guard + soundness controls rejected by all three): $([ "$ok" = 1 ] && echo PASS || echo FAIL)"
[ "$ok" = 1 ]
