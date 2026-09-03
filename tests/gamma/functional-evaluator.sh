#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
EXPERIMENT="$OMEGA_REPO_ROOT/tests/gamma/evaluator-development"

materialize_beta_compiler "$TMP/beta" >/dev/null
"$TMP/beta" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
cmp "$TMP/evaluator.tape" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"
python3 "$EXPERIMENT/resolve.py" "$EXPERIMENT/gamma_evaluator.sbeta" \
    "$TMP/reconstructed.beta"
cmp "$TMP/reconstructed.beta" "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE"

"$EXPERIMENT/run.sh" >/dev/null
echo "Functional Gamma evaluator: Beta reconstruction and scalar/effect augmentation passed"
