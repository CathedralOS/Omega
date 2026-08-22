#!/usr/bin/env sh
# CHECKER-DIAMOND FUZZER — broad random coverage of the two trust-anchor checkers agreeing. checker-
# diamond.sh cross-checks check.beta (Beta) and checker.gamma (Gamma, on the reference interpreter) at
# ~83 curated certificates; this generates hundreds of random closed Peano/List equality propositions
# (with refl proofs) and requires BOTH checkers to ACCEPT the true ones and REJECT the perturbed ones.
# A disagreement is a bug (or a backdoor) in one of the two independent checkers -- the most important
# place the lattice's "trust by checking" thesis can fail. Deterministic (fixed seed). Needs python3.
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
command -v python3 >/dev/null 2>&1 || { echo "checker-diamond fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# Third oracle: the type-checked checker (checker_typed.gamma, which typeck.beta accepts), mechanically
# type-erased to what the interpreter runs -- so "the checker the type system validates" is fuzzed too.
TYPED=""
if python3 "${OMEGA_PATH_GAMMA}"/erase_types.py < "${OMEGA_PATH_GAMMA}"/checker_typed.gamma > "$T/erased.gamma" 2>/dev/null; then
  TYPED="$T/erased.gamma"
fi
python3 checker-diamond-fuzz.py "$T/check.exe" "$T/interp.exe" "${OMEGA_PATH_GAMMA}"/checker.gamma "$TYPED" "${1:-60}"
