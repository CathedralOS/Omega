#!/usr/bin/env sh
# Materialize the canonical Gamma proof-source closures: the derivation
# checker implementation and the Beta encoding theory.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_DERIVATION_CHECKER_SOURCES:-}" ] && \
  [ -n "${OMEGA_PATH_BETA_ENCODING_SOURCES:-}" ] && \
  [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Proof sources: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound proof-source subjects. These are ordinary Gamma member closures, not
# tapes or entries: each GammaSourceClosureV1 manifest is the ordered source
# authority whose rows bind every member length, digest, and path, and
# repacking it reproduces the packed member bytes pinned in the owning proof
# README. Test gates still supply their own diagnostic prefix entries; the
# bound closure carries members only. A digest here is an identity check that
# the bytes being materialized are the bound ones; it is not a proof of the
# checker, the theory, or any derived certificate. Changing a member or a
# manifest invalidates the dependent evidence and must update every record.
DERIVATION_CHECKER_MANIFEST_SIZE=9046
DERIVATION_CHECKER_MANIFEST_SHA256=661f3483b149bfed17cc9b994aac5dc377061c95f7f0d1bc32e3b620dae70165
DERIVATION_CHECKER_PACKED_SIZE=62349
DERIVATION_CHECKER_PACKED_SHA256=6423e10ca5dab533d8d0f58dc1e66eb08917273889a00d5da985433483528802
BETA_ENCODING_MANIFEST_SIZE=3600
BETA_ENCODING_MANIFEST_SHA256=757cf25f1eed02d65437945ec7f9fd984810fad5041cd7fc39a742565cc23dcb
BETA_ENCODING_PACKED_SIZE=21305
BETA_ENCODING_PACKED_SHA256=26dd7d6bd28f07222a34eec33067ee2a9efd77f0b3359e43b45816b3deb06d4d

# require_bound_manifest_closure LABEL MANIFEST SIZE SHA256 PACKED_SIZE
# PACKED_SHA256 RECORD : shared manifest-then-repack check behind both proof
# subjects. Repacking needs python3; without it the closure identity cannot
# be established and the check refuses rather than skipping.
require_bound_manifest_closure() {
  require_bound_identity "$1" "$2" "$3" "$4" "$7" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the $1 source closure" >&2
    return 2
  }
  PROOF_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$2" "$PROOF_IDENTITY_TMP/closure.gamma" || {
      PROOF_IDENTITY_RC=$?
      rm -rf -- "$PROOF_IDENTITY_TMP"
      return "$PROOF_IDENTITY_RC"
    }
  require_bound_identity "canonical $1 closure" \
    "$PROOF_IDENTITY_TMP/closure.gamma" "$5" "$6" "$7"
  PROOF_IDENTITY_RC=$?
  rm -rf -- "$PROOF_IDENTITY_TMP"
  return "$PROOF_IDENTITY_RC"
}

# require_derivation_checker_identity / require_beta_encoding_theory_identity :
# the canonical manifest is the bound file and repacking it reproduces exactly
# the bound member closure. Every materialization runs its check; tests may
# call them directly. bootstrap_sha256 and require_bound_identity live in
# alpha/seed_env.sh.
require_derivation_checker_identity() {
  require_bound_manifest_closure "implementation.gamma.sources" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$DERIVATION_CHECKER_MANIFEST_SIZE" "$DERIVATION_CHECKER_MANIFEST_SHA256" \
    "$DERIVATION_CHECKER_PACKED_SIZE" "$DERIVATION_CHECKER_PACKED_SHA256" \
    "bootstrap/proofs/checker/README.md"
}

require_beta_encoding_theory_identity() {
  require_bound_manifest_closure "theory.gamma.sources" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" \
    "$BETA_ENCODING_MANIFEST_SIZE" "$BETA_ENCODING_MANIFEST_SHA256" \
    "$BETA_ENCODING_PACKED_SIZE" "$BETA_ENCODING_PACKED_SHA256" \
    "bootstrap/proofs/beta_encoding/README.md"
}

# materialize_derivation_checker DEST / materialize_beta_encoding_theory DEST :
# write the canonical packed member Gamma source to DEST after the bound
# identity check. source_closure.py checks every member row and writes
# atomically; a changed member refuses before DEST is written.
materialize_derivation_checker() {
  DERIVATION_CHECKER_DEST=$1
  require_derivation_checker_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" "$DERIVATION_CHECKER_DEST"
}

materialize_beta_encoding_theory() {
  BETA_ENCODING_DEST=$1
  require_beta_encoding_theory_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" "$BETA_ENCODING_DEST"
}
