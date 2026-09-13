#!/usr/bin/env sh
# Materialize the selected Beta-authored Gamma evaluator.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_GAMMA_EVALUATOR_TAPE:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Gamma evaluator: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Beta-to-Gamma edge subject. These are the same identities pinned by
# bootstrap/2_gamma/EVALUATOR_PROFILE.md ("The selected implementation is"),
# repeated in tests/gamma/heap-boundary/evaluator.tsv and required by
# bootstrap/3_delta/delta_compiler.composed. A digest here is an identity
# check that the bytes being stamped are the selected evaluator; it is not a
# proof that the evaluator implements Gamma. Changing the source or tape
# invalidates the dependent evidence and must update every record.
GAMMA_EVALUATOR_SOURCE_SIZE=47748
GAMMA_EVALUATOR_SOURCE_SHA256=8b4d2b8d27fb6ab23bd732abf6615012b92c739fb6d7cdb220dbd557c1d8925f
GAMMA_EVALUATOR_TAPE_SIZE=8575
GAMMA_EVALUATOR_TAPE_SHA256=ad55c3f18d3c7bd3e1189635bf34ff6595ca97c34afe85412a6127ed2d29e015

# require_gamma_evaluator_identity : the canonical source and tape are exactly
# the selected pair. Every materialization runs it; tests may call it
# directly. bootstrap_sha256 and require_bound_identity live in
# alpha/seed_env.sh.
require_gamma_evaluator_identity() {
  require_bound_identity "gamma_evaluator.beta" "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" \
    "$GAMMA_EVALUATOR_SOURCE_SIZE" "$GAMMA_EVALUATOR_SOURCE_SHA256" \
    "bootstrap/2_gamma/EVALUATOR_PROFILE.md" || return $?
  require_bound_identity "gamma_evaluator_bytecode.tape" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    "$GAMMA_EVALUATOR_TAPE_SIZE" "$GAMMA_EVALUATOR_TAPE_SHA256" \
    "bootstrap/2_gamma/EVALUATOR_PROFILE.md" || return $?
}

materialize_gamma_evaluator() {
  GAMMA_EVALUATOR_DEST=$1
  require_gamma_evaluator_identity || return $?
  stamp_seed "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$GAMMA_EVALUATOR_DEST"
}
