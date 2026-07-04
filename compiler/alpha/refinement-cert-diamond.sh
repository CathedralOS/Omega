#!/usr/bin/env sh
# REFINEMENT-CERT DIAMOND — diverse double-checking of the refinement gate's certificates.
#
# The instruction-level refinement gate proves `bc(P) ≡ P` with refl certificates over the meaning
# language's CONSTRUCTOR families — (k 5 ..) ℤ pairs, (k 6 ..) monus, (k 7/8 ..) the input stream, and the
# (f 90 ..) triangular recurrence. Those certs were validated by check.beta ALONE; the lattice's diversity
# thesis says every certificate class must be decided identically by an INDEPENDENT checker. This gate
# re-runs every cert the refinement gate produces (accepts AND the perturbed teeth rejects) through
# check_ref.py — the independent, auditable reference checker — and requires verdict-for-verdict agreement.
# (checker.gamma has no user-constructor support yet, the same known gap as its missing def/use lemma
# prelude — the gamma leg joins when that rung grows constructors.)
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "refinement-cert-diamond: skipped (python3 absent)"; exit 0; }
. seed_env.sh
SEED=$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < ../delta/check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "check.beta build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < ../gamma/interp.beta > "$T/i.asm" 2>/dev/null && "$ASM" < "$T/i.asm" > "$T/i.tape" 2>/dev/null \
  && stamp_seed "$T/i.tape" "$SEED" "$T/interp.exe" >/dev/null 2>&1 || { echo "interp.beta build failed"; exit 1; }
DEFS=$(cat ../gamma/checker.gamma)

mkdir "$T/certs"
# run the gate over the curated samples only (fuzz 0) with cert emission; its own pass/fail still applies
REFINE_CERT_DIR="$T/certs" REFINE_FUZZ=0 REFINE_LOOP_FUZZ=0 REFINE_COMPOSE_FUZZ=0 REFINE_NESTED_FUZZ=0 \
  python3 alpha_refinement_check.py "$T/check.exe" "$(pwd)/../beta-lang-rs/build/bc.exe" "$(pwd)/$ASM" >/dev/null \
  || { echo "refinement gate failed during cert emission"; exit 1; }

PASS=0; FAIL=0; GPASS=0; GFAIL=0
for c in "$T"/certs/cert-*.beta; do
  [ -f "$c" ] || continue
  expect=$(basename "$c" .beta | sed 's/.*-//')
  got=$(python3 ../delta/check_ref.py < "$c" 2>/dev/null || echo error)
  if [ "$got" = "$expect" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $(basename "$c") : check.beta=$expect check_ref.py=$got"; fi
  # THIRD leg: checker.gamma — the cert translated to its (check ..) syntax ((k ..) terms become the CURRIED
  # constructor encoding (Apply.. (Con cid) ..); (fun ..) rules are inlined at each (f ..) site as Fapp).
  gexpr=$(python3 ../gamma/refcert_to_gamma.py < "$c" 2>/dev/null) || { GFAIL=$((GFAIL+1)); echo "  FAIL $(basename "$c") : untranslatable to checker.gamma"; continue; }
  vg=0; printf '%s\n%s\n' "$DEFS" "$gexpr" | "$T/interp.exe" >/dev/null 2>&1 || vg=$?   # accept = exit 1
  gv=reject; [ "$vg" = 1 ] && gv=accept
  if [ "$gv" = "$expect" ]; then GPASS=$((GPASS+1))
  else GFAIL=$((GFAIL+1)); echo "  FAIL $(basename "$c") : check.beta=$expect checker.gamma=$gv"; fi
done
echo "refinement-cert diamond (every refl cert decided identically by check.beta, check_ref.py AND checker.gamma): $PASS+$GPASS ok, $((FAIL+GFAIL)) failed"
[ "$FAIL" = 0 ] && [ "$GFAIL" = 0 ] && [ "$PASS" -gt 0 ]
