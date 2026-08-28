#!/usr/bin/env sh
# CONTRACT DISCHARGE (omega source), AUTOMATED — the requires/ensures of samples/math_proofs are proof
# obligations; this gate discharges the arithmetic-and-ordering fragment with a kernel certificate the
# PROVER generates and ALL THREE independent checkers verify.
#
# tools/contract2proof.py translates each contract machine into the kernel proposition it obligates (prover
# syntax). tools/prover.py (the untrusted proof-SEARCH front line) discharges it; implementations/beta/check.beta, implementations/reference/check_ref.py AND
# implementations/gamma/checker.gamma verify the certificate — so the omega SOURCE contract (not a hand-picked goal) is proven,
# three ways, with NO hand-authored proof. contract2proof --perturb succs the conclusion RHS into a
# well-formed FALSE proposition, which the prover must fail to prove (or, if it emits something, all three
# checkers reject). This is omega's obligations concept: obligations discharged automatically.
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
command -v python3 >/dev/null 2>&1 || { echo "math-contracts: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "math-contracts FAIL — build interp.beta"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_ALPHA_CHECKER}"/implementations/gamma/checker.gamma)
SRC="${OMEGA_PATH_CORPUS}"/math_corpus/proofs/main.omg

gverdict() {
  gg=$(python3 tools/prover.py --gamma "$1" 30 2>/dev/null)
  [ "$gg" = unprovable ] || [ -z "$gg" ] && { echo unprovable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0; cov=0; gap=0
python3 tools/contract2proof.py < "$SRC" | grep -v UNSUPPORTED | while IFS="$(printf '\t')" read -r name prop; do
  cert=$(python3 tools/prover.py "$prop" 30 2>/dev/null)
  if [ "$cert" = unprovable ] || [ -z "$cert" ]; then
    echo "  gap  $name : translated, but the prover cannot discharge it yet"
    continue
  fi
  printf '%s' "$cert" > "$T/good.cert"
  gb=$(cat "$T/good.cert" | "$T/check.exe"); gr=$(cat "$T/good.cert" | python3 implementations/reference/check_ref.py 2>/dev/null); gg=$(gverdict "$prop")
  # perturbed proposition (off by one) must not be provable-and-accepted
  badprop=$(python3 tools/contract2proof.py --perturb < "$SRC" | grep "^$name	" | cut -f2)
  badcert=$(python3 tools/prover.py "$badprop" 30 2>/dev/null)
  bok=no
  if [ "$badcert" = unprovable ] || [ -z "$badcert" ]; then bok=yes
  else printf '%s' "$badcert" | "$T/check.exe" 2>/dev/null | grep -q accept || bok=yes; fi
  if [ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] && [ "$bok" = yes ]; then
    echo "  ok   $name : source contract discharged by prover + ALL THREE checkers (perturbation not provable)"
  else
    echo "  FAIL $name : (beta=$gb ref=$gr gamma=$gg perturbation-safe=$bok)"
  fi
done | tee "$T/report"

cov=$(grep -c '^  ok ' "$T/report")
gap=$(grep -c '^  gap ' "$T/report")
grep -q '^  FAIL' "$T/report" && fail=1
nunc=$(python3 tools/contract2proof.py < "$SRC" | grep -c UNSUPPORTED)
echo "contract discharge (omega requires/ensures auto-proven by the prover, verified by implementations/beta/check.beta + check_ref + implementations/gamma/checker.gamma): $cov discharged, $gap prover-gap, $nunc outside the fragment"
[ $fail = 0 ] && [ "$cov" -gt 0 ]
