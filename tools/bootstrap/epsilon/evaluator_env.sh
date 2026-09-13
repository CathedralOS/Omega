#!/usr/bin/env sh
# Materialize the canonical Delta-authored Epsilon evaluator closure.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_EPSILON_COMPILER_SOURCES:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Epsilon evaluator: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Delta-to-Epsilon edge subject. The evaluator is not a tape:
# epsilon_compiler.delta.sources is the ordered source authority whose
# DeltaSourceClosureV1 rows bind every member length, digest, and path, and
# repacking it reproduces the packed evaluator bytes pinned in
# bootstrap/4_epsilon/README.md ("Exact source closure"). A digest here is an
# identity check that the bytes being materialized are the bound ones; it is
# not a proof that the evaluator implements Epsilon. Changing a member or the
# manifest invalidates the dependent evidence and must update every record.
EPSILON_EVALUATOR_MANIFEST_SIZE=15163
EPSILON_EVALUATOR_MANIFEST_SHA256=717e6bdc90850b9cacc0858279f3483d10c93f3289f43d9ff83ac27b4e862061
EPSILON_EVALUATOR_PACKED_SIZE=617354
EPSILON_EVALUATOR_PACKED_SHA256=4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e

# require_epsilon_evaluator_identity : the canonical manifest is the bound
# file and repacking it reproduces exactly the bound evaluator closure.
# Every materialization runs it; tests may call it directly. bootstrap_sha256
# and require_bound_identity live in alpha/seed_env.sh. Repacking needs
# python3; without it the closure identity cannot be established and the
# check refuses rather than skipping.
require_epsilon_evaluator_identity() {
  require_bound_identity "epsilon_compiler.delta.sources" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" \
    "$EPSILON_EVALUATOR_MANIFEST_SIZE" "$EPSILON_EVALUATOR_MANIFEST_SHA256" \
    "bootstrap/4_epsilon/README.md" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the Epsilon source closure" >&2
    return 2
  }
  EPSILON_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON_IDENTITY_TMP/evaluator.delta" || {
      EPSILON_IDENTITY_RC=$?
      rm -rf -- "$EPSILON_IDENTITY_TMP"
      return "$EPSILON_IDENTITY_RC"
    }
  require_bound_identity "canonical Epsilon evaluator closure" \
    "$EPSILON_IDENTITY_TMP/evaluator.delta" \
    "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256" \
    "bootstrap/4_epsilon/README.md"
  EPSILON_IDENTITY_RC=$?
  rm -rf -- "$EPSILON_IDENTITY_TMP"
  return "$EPSILON_IDENTITY_RC"
}

# materialize_epsilon_evaluator DEST : write the canonical packed evaluator
# Delta source to DEST after the bound identity check. source_closure.py
# checks every member row and writes atomically; a changed member refuses
# before DEST is written.
materialize_epsilon_evaluator() {
  EPSILON_EVALUATOR_DEST=$1
  require_epsilon_evaluator_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON_EVALUATOR_DEST"
}
