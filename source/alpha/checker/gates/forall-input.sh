#!/usr/bin/env sh
# ∀-INPUT THEOREM — the summit's per-vector input proofs made UNIVERSAL, and MECHANICAL.
#
# count/k-family: fold gains a constant per element. sum-forall: fold gains the ELEMENT (needs uadd
# proven a commutative monoid). input-tv.sh proves an input loop's meaning on particular input vectors. This gate proves it for ALL
# inputs at once — ∀xs ∀n. fold(xs, n) = agg(xs) + n — by structural induction on xs, the induction
# hypothesis instantiated at the SHIFTED accumulator (the shape recx was added to the kernel to express).
#
# corpus/count-forall.elab is the hand-authored k=1 (count/len) reference; tools/forall-gen.py MECHANICALLY emits the
# whole constant-increment family (fold that adds k successors per element -> k*len), proving the proof
# was a reusable SCHEMA. Every theorem must be accepted by ALL THREE independent checkers (implementations/beta/check.beta,
# implementations/reference/check_ref.py, implementations/gamma/checker.gamma via refcert_to_gamma on interp.beta), and an off-by-one perturbed goal must
# be rejected by all three — so acceptance is meaningful.
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
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
. "$OMEGA_PATH_PROOF_KERNEL/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "forall-input: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "forall-input FAIL — build interp.beta"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma)

gverdict() {  # translate a cert to implementations/gamma/checker.gamma, run it; echoes accept/reject/undecided
  gg=$(python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/refcert_to_gamma.py < "$1" 2>/dev/null) || { echo untranslatable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0
# verify one elab SOURCE (on stdin via $1 command): theorem accepted by all three, perturbation rejected.
verify() {  # $1 = label, $2 = shell command emitting the .elab source
  eval "$2" > "$T/src.elab"
  python3 tools/elab.py < "$T/src.elab" > "$T/good.cert" 2>/dev/null || { echo "  FAIL $1: elaboration errored"; fail=1; return; }
  ab=$(cat "$T/good.cert" | "$T/check.exe"); ar=$(cat "$T/good.cert" | python3 implementations/reference/check_ref.py); ag=$(gverdict "$T/good.cert")
  [ "$ab" = accept ] && [ "$ar" = accept ] && [ "$ag" = accept ] \
    && echo "  ok   $1 accepted by all three (implementations/beta/check.beta/check_ref/implementations/gamma/checker.gamma)" \
    || { echo "  FAIL $1: not accepted by all three (beta=$ab ref=$ar gamma=$ag)"; fail=1; }
  # PERTURB: wrap the goal RHS in one extra successor; the proof must no longer fit. $3 overrides the sed for
  # theorems whose goal shape differs (e.g. the two-accumulator fold's paired RHS).
  sed "${3:-/^(all xs/ s/(f 21 (f 9\([0-9]\) xs) n)/(k 3 (f 21 (f 9\1 xs) n))/}" "$T/src.elab" \
    | python3 tools/elab.py > "$T/bad.cert" 2>/dev/null
  pb=$(cat "$T/bad.cert" | "$T/check.exe"); pr=$(cat "$T/bad.cert" | python3 implementations/reference/check_ref.py); pg=$(gverdict "$T/bad.cert")
  [ "$pb" = reject ] && [ "$pr" = reject ] && [ "$pg" = reject ] \
    && echo "  ok   $1 perturbation (off by one) rejected by all three" \
    || { echo "  FAIL $1: perturbation not rejected by all three (beta=$pb ref=$pr gamma=$pg)"; fail=1; }
}

verify "count-forall (hand-authored k=1)" "cat corpus/count-forall.elab"
verify "generated k=1 (count -> len)"     "python3 tools/forall-gen.py 1"
verify "generated k=2 (2 per elem -> 2*len)" "python3 tools/forall-gen.py 2"
verify "generated k=3 (3 per elem -> 3*len)" "python3 tools/forall-gen.py 3"
verify "sum-forall (add-the-element fold; uadd commutative monoid)" "cat corpus/sum-forall.elab"
verify "pair-forall (TWO-accumulator fold: sum AND count threaded together, via a pair + pair congruence)" \
  "cat corpus/pair-forall.elab" '/^(all xs/ s|(k 70 (f 21 (f 94 xs) s)|(k 70 (k 3 (f 21 (f 94 xs) s))|'
verify "prod-forall (MULTIPLICATIVE fold: prodfold(xs,n)=listprod(xs)*n; umul a commutative semiring)" \
  "cat corpus/prod-forall.elab" '/^(all xs/ s|(f 22 (f 98 xs) n)|(k 3 (f 22 (f 98 xs) n))|'
verify "listsum-hom (MapReduce law: listsum(xs++ys) = listsum(xs)+listsum(ys); divide-and-conquer aggregation)" \
  "cat corpus/listsum-hom.elab" '/^(all xs/ s|(f 21 (f 94 xs) (f 94 ys))|(k 3 (f 21 (f 94 xs) (f 94 ys)))|'
verify "len-hom (count MapReduce law: len(xs++ys) = len(xs)+len(ys))" \
  "cat corpus/len-hom.elab" '/^(all xs/ s|(f 21 (f 97 xs) (f 97 ys))|(k 3 (f 21 (f 97 xs) (f 97 ys)))|'
verify "max-forall (ORDER-monoid fold: maxfold(xs,acc)=max(acc,listmax xs); max = the ≤-lattice JOIN, dual-dispatch mutual recursion, associative monoid with identity 0)" \
  "cat corpus/max-forall.elab" '/^(all xs/ s|(f 23 acc (f 94 xs))|(k 3 (f 23 acc (f 94 xs)))|'
verify "sqsum-forall (SQUARE fold: sqfold(xs,n)=sumSq(xs)+n where sumSq adds each element's SQUARE h*h; umul for the square, same uadd commutative monoid as sum-forall)" \
  "cat corpus/sqsum-forall.elab"

echo "forall-input theorem (per-vector input proofs made universal AND mechanical; all three checkers agree, perturbations rejected): $( [ $fail = 0 ] && echo PASS || echo FAIL )"
[ $fail = 0 ]
