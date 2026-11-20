#!/usr/bin/env sh
# Materialize the selected Beta-authored Gamma evaluator.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_GAMMA_EVALUATOR_TAPE:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Gamma evaluator: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Beta-to-Gamma edge subject. These are the same identities pinned by
# bootstrap/gamma/interpreter/EVALUATOR_PROFILE.md ("The selected implementation is"),
# required by
# bootstrap/delta/compiler/delta_compiler.composed. A digest here is an identity
# check that the bytes being stamped are the selected evaluator; it is not a
# proof that the evaluator implements Gamma. Changing the source or tape
# invalidates the dependent evidence and must update every record.
GAMMA_EVALUATOR_SOURCE_SIZE=48043
GAMMA_EVALUATOR_SOURCE_SHA256=3a41a36bb5af68454a92fda0e3c5dd6b78784cb62aa9bc2eb10e2cc6ddc53f71
GAMMA_EVALUATOR_TAPE_SIZE=8447
GAMMA_EVALUATOR_TAPE_SHA256=d6265bba615fab3bd7d3fd3432f416e28c5668e17119d1970e9d3960b315fe4b

# require_gamma_evaluator_identity : the canonical source and tape are exactly
# the selected pair. Every materialization runs it; tests may call it
# directly. bootstrap_sha256 and require_bound_identity live in
# alpha/seed_env.sh.
require_gamma_evaluator_identity() {
  require_bound_identity "gamma_evaluator.beta" "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" \
    "$GAMMA_EVALUATOR_SOURCE_SIZE" "$GAMMA_EVALUATOR_SOURCE_SHA256" \
    "bootstrap/gamma/interpreter/EVALUATOR_PROFILE.md" || return $?
  require_bound_identity "gamma_evaluator_bytecode.tape" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    "$GAMMA_EVALUATOR_TAPE_SIZE" "$GAMMA_EVALUATOR_TAPE_SHA256" \
    "bootstrap/gamma/interpreter/EVALUATOR_PROFILE.md" || return $?
}

materialize_gamma_evaluator() {
  GAMMA_EVALUATOR_DEST=$1
  require_gamma_evaluator_identity || return $?
  stamp_seed "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$GAMMA_EVALUATOR_DEST"
}
