#!/usr/bin/env sh
# Materialize the selected Beta-authored Gamma evaluator.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_GAMMA_EVALUATOR_TAPE:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Gamma evaluator: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

materialize_gamma_evaluator() {
  GAMMA_EVALUATOR_DEST=$1
  [ -f "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" ] || {
    echo "Gamma evaluator: missing $OMEGA_PATH_GAMMA_EVALUATOR_TAPE" >&2
    return 2
  }
  stamp_seed "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$GAMMA_EVALUATOR_DEST"
}
