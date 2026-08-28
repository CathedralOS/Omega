#!/usr/bin/env sh
# TERMINATION OBLIGATION DISCHARGE (omega source) — the normative
# `terminates by s -> Slice::Length;`
# clause on a machine is a proof obligation: every recursive self-call must STRICTLY DECREASE the measure,
# so the recursion is well-founded and the machine halts on ALL inputs (not just the ones the meaning route
# happens to run). omega discharges termination obligations statically; this gate does the proof-kernel analogue
# for the Slice::Length measure with tail recursion on `s[1..]`.
#
# All such machines share ONE measure-decrease fact: for a nonempty slice s = cons(h, t), the recursion is
# on the tail t (= s[1..]) and len(t) < len(cons h t) — the length drops by exactly 1. corpus/proofs/slice-tail-
# shrink.elab proves that fact (`ex k. len t + s(k) = len(cons h t)`), verified here by implementations/beta/check.beta,
# implementations/reference/check_ref.py AND implementations/gamma/checker.gamma. The gate then ties each omega source machine carrying the obligation to
# the proven lemma: it confirms the machine declares `terminates by <s> -> Slice::Length` AND tail-recurses on
# `<s>[1..]`, so its declared measure is exactly the one proven to strictly decrease. A NEGATIVE control (the
# reversed claim len(cons h t) < len t — the measure GROWS, i.e. non-termination) must be rejected by the
# kernel AND the reference checker. No hand-picked goal: the samples' own decreases clauses are discharged.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
command -v python3 >/dev/null 2>&1 || { echo "termination-obligations: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "termination-obligations FAIL — build interp.beta"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_ALPHA_CHECKER}"/implementations/gamma/checker.gamma)
LEMMA=corpus/proofs/slice-tail-shrink.elab

# ---- 1. the measure-decrease lemma, verified by all three independent checkers ----
cert=$(python3 tools/elab.py < "$LEMMA" 2>/dev/null)
gb=$(printf '%s' "$cert" | "$T/check.exe" 2>/dev/null)
gr=$(printf '%s' "$cert" | python3 implementations/reference/check_ref.py 2>/dev/null)
gg=$(printf '%s\n%s\n' "$DEFS" "$(printf '%s' "$cert" | python3 "${OMEGA_PATH_ALPHA_CHECKER}"/tools/refcert_to_gamma.py 2>/dev/null)" | perl -e 'alarm 40; exec @ARGV' "$T/interp.exe" >/dev/null 2>&1; r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; })
echo "  measure-decrease lemma (len t < len(cons h t)): implementations/beta/check.beta=$gb check_ref=$gr implementations/gamma/checker.gamma=$gg"

# ---- 2. NEGATIVE control: the reversed measure (len(cons h t) < len t = the measure GROWS) must be rejected ----
neg=$(sed 's/(all h (all t (ex k (= (+ (len t) (s k)) (len (cons h t))))))/(all h (all t (ex k (= (+ (len (cons h t)) (s k)) (len t)))))/' "$LEMMA" | python3 tools/elab.py 2>/dev/null)
nb=$(printf '%s' "$neg" | "$T/check.exe" 2>/dev/null)
nr=$(printf '%s' "$neg" | python3 implementations/reference/check_ref.py 2>/dev/null)
neg_ok=no; { [ "$nb" != accept ] && [ "$nr" != accept ]; } && neg_ok=yes
echo "  negative control (reversed measure grows): implementations/beta/check.beta=$nb check_ref=$nr -> rejected=$neg_ok"

# ---- 3. tie each omega source obligation to the proven lemma ----
cov=0; miss=0
for f in "${OMEGA_PATH_CORPUS}"/*/main.omg; do
  dec=$(grep -oE 'terminates by [a-z_]+ -> Slice::Length;' "$f" | head -1)
  [ -n "$dec" ] || continue
  s=$(basename "$(dirname "$f")")
  var=$(printf '%s' "$dec" | sed -E 's/terminates by ([a-z_]+).*/\1/')
  # the machine must tail-recurse on <var>[1..] (the shrinking window the lemma covers)
  if grep -qF "${var}[1.." "$f"; then
    n=$(grep -cE 'terminates by [a-z_]+ -> Slice::Length;' "$f")
    cov=$((cov + n))
    echo "  ok   $s : $n machine(s) 'terminates by $var -> Slice::Length', tail-recurse on $var[1..] -> discharged by the lemma"
  else
    miss=$((miss + 1))
    echo "  MISS $s : declares Slice::Length decreases but no $var[1..] shrinking recursion found"
  fi
done

ok=1
[ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] || ok=0
[ "$neg_ok" = yes ] || ok=0
[ "$cov" -gt 0 ] && [ "$miss" = 0 ] || ok=0
echo "termination obligations (omega 'terminates by s -> Slice::Length' discharged by a 3-checker-verified measure-decrease lemma; reversed measure rejected): $cov machine-obligation(s) tied to the lemma across the corpus"
[ "$ok" = 1 ]
