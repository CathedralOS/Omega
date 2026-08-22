#!/usr/bin/env sh
# REFINEMENT-CERT CROSS-CHECK — replay refinement certificates across implementations.
#
# The instruction-level refinement gate proves `bc(P) ≡ P` with refl certificates over the meaning
# language's CONSTRUCTOR families — (k 5 ..) ℤ pairs, (k 6 ..) monus, (k 7/8 ..) the input stream, (k 9..13)
# cond/booleans, (k 14/15) div/mod, and the (f 90 ..) triangular recurrence. As regression evidence, this gate re-runs every
# cert the refinement gate produces (accepts AND the perturbed teeth rejects) through BOTH check_ref.py (the
# auditable reference checker) AND checker.gamma (the gamma-language checker, via refcert_to_gamma.py, which
# maps (k CID args) to the curried (Apply.. (Con CID) ..) constructor encoding — Con/Apply thread through
# pnorm/nateq/subt/freet, so no gamma feature was needed). All THREE must agree verdict-for-verdict; this is a
# bug-finding cross-check, not DDC or a replacement for the soundness bridge. (The
# straight-line fuzz certs go through prover.py; the prover-diamond triple-checks those separately.)
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
command -v python3 >/dev/null 2>&1 || { echo "refinement-cert-diamond: skipped (python3 absent)"; exit 0; }
. seed_env.sh
SEED=$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null 2>&1 ) || { echo "bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "${OMEGA_PATH_PROOF_KERNEL}"/check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "check.beta build failed"; exit 1; }
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "${OMEGA_PATH_GAMMA}"/interp.beta > "$T/i.asm" 2>/dev/null && "$ASM" < "$T/i.asm" > "$T/i.tape" 2>/dev/null \
  && stamp_seed "$T/i.tape" "$SEED" "$T/interp.exe" >/dev/null 2>&1 || { echo "interp.beta build failed"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_GAMMA}"/checker.gamma)

mkdir "$T/certs"
# run the gate over the curated samples PLUS a deterministic slice of the loop/nested fuzz spaces (their
# refl-path certs exercise every constructor family; straight-line fuzz certs go through prover.py, whose
# certs the prover-diamond already double-checks). The gate seeds its RNG, so the cert set is stable.
REFINE_CERT_DIR="$T/certs" REFINE_FUZZ=0 REFINE_LOOP_FUZZ=6 REFINE_COMPOSE_FUZZ=0 REFINE_NESTED_FUZZ=4 \
  python3 alpha_refinement_check.py "$T/check.exe" "${OMEGA_PATH_BETA_RUST}/build/bc.exe" "$ASM" >/dev/null \
  || { echo "refinement gate failed during cert emission"; exit 1; }

PASS=0; FAIL=0; GPASS=0; GFAIL=0
for c in "$T"/certs/cert-*.beta; do
  [ -f "$c" ] || continue
  expect=$(basename "$c" .beta | sed 's/.*-//')
  got=$(python3 "${OMEGA_PATH_PROOF_KERNEL}"/check_ref.py < "$c" 2>/dev/null || echo error)
  if [ "$got" = "$expect" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $(basename "$c") : check.beta=$expect check_ref.py=$got"; fi
  # THIRD leg: checker.gamma — the cert translated to its (check ..) syntax ((k ..) terms become the CURRIED
  # constructor encoding (Apply.. (Con cid) ..); (fun ..) rules are inlined at each (f ..) site as Fapp).
  gexpr=$(python3 "${OMEGA_PATH_GAMMA}"/refcert_to_gamma.py < "$c" 2>/dev/null) || { GFAIL=$((GFAIL+1)); echo "  FAIL $(basename "$c") : untranslatable to checker.gamma"; continue; }
  vg=0; printf '%s\n%s\n' "$DEFS" "$gexpr" | "$T/interp.exe" >/dev/null 2>&1 || vg=$?   # accept = exit 1
  gv=reject; [ "$vg" = 1 ] && gv=accept
  if [ "$gv" = "$expect" ]; then GPASS=$((GPASS+1))
  else GFAIL=$((GFAIL+1)); echo "  FAIL $(basename "$c") : check.beta=$expect checker.gamma=$gv"; fi
done
echo "refinement-cert diamond (every refl cert decided identically by check.beta, check_ref.py AND checker.gamma): $PASS+$GPASS ok, $((FAIL+GFAIL)) failed"
[ "$FAIL" = 0 ] && [ "$GFAIL" = 0 ] && [ "$PASS" -gt 0 ]
