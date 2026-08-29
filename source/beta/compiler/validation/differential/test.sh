#!/usr/bin/env sh
# Bounded symbolic differential for the canonical Beta compiler. This is an
# optional drift detector, not an admission certificate or lattice stage.
set -e
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
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "symbolic differential: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA_CHECKER}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}/$ALPHA_SEED"
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
BC="$T/bc.exe"
"$ASM" < "$OMEGA_PATH_BETA_COMPILER_SOURCE" > "$T/bc.tape"
stamp_seed "$T/bc.tape" "$SEED" "$BC" >/dev/null 2>&1 || { echo "symbolic differential: compiler construction failed"; exit 1; }
stamp_proof_checker "$T/check.exe" >/dev/null 2>&1 || { echo "symbolic differential: checker artifact unavailable"; exit 1; }

echo "bounded symbolic differential (canonical compiler output versus source model):"
python3 "$OMEGA_GATE_DIR/symbolic_differential.py" \
  "$T/check.exe" "$BC"
