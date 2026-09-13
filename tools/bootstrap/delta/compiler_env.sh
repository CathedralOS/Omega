#!/usr/bin/env sh
# Materialize the canonical Gamma-authored Delta compiler closure.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_DELTA_COMPILER_SOURCES:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Delta compiler: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

# Bound Gamma-to-Delta edge subjects. The canonical compiler is not a tape:
# delta_compiler.gamma is the DCREQ request entry, implementation.gamma.sources
# is the ordered source authority, and delta_compiler.composed binds the packed
# entry-plus-member bytes and selected evaluator tape under GammaComposedV1
# (bootstrap/2_gamma/COMPOSED_ARTIFACT.md). The packed closure identity is the
# same source-sha256/source-length the composed record and the staged-compiler
# and normalization compiler.tsv rows pin. A digest here is an identity check
# that the bytes being materialized are the bound ones; it is not a proof of
# the compiler. Changing any member, the entry, or the manifest invalidates
# the dependent evidence and must update every record.
DELTA_COMPILER_ENTRY_SIZE=717
DELTA_COMPILER_ENTRY_SHA256=b4edbdaa38f2c308178bcf24a368203c5d30780149ed7f06c4f481dd0b4ec5dd
DELTA_COMPILER_MANIFEST_SIZE=11136
DELTA_COMPILER_MANIFEST_SHA256=376c42e10f5e6d785533d57d5f662b46431547015eebe6097878aa22a03e3311
DELTA_COMPILER_COMPOSED_SIZE=198
DELTA_COMPILER_COMPOSED_SHA256=8ac7a3f8b606baf4fbb6fe12a684ac80260c639bef75539e33d47fee6fa459d8
DELTA_COMPILER_PACKED_SIZE=155440
DELTA_COMPILER_PACKED_SHA256=65e23e66c57885382a90c5b910a9064d8c829d62ffd1f028091d22e32a8eca84

# require_delta_compiler_identity : the canonical entry, manifest, and
# composed record are the bound files; the composed record names the selected
# Gamma evaluator and packed closure; and repacking the manifest reproduces
# exactly the bound closure. Every materialization runs it; tests may call it
# directly. bootstrap_sha256 and require_bound_identity live in
# alpha/seed_env.sh. Repacking needs python3; without it the closure identity
# cannot be established and the check refuses rather than skipping.
require_delta_compiler_identity() {
  require_bound_identity "delta_compiler.gamma" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCE" \
    "$DELTA_COMPILER_ENTRY_SIZE" "$DELTA_COMPILER_ENTRY_SHA256" \
    "bootstrap/3_delta/README.md" || return $?
  require_bound_identity "implementation.gamma.sources" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" \
    "$DELTA_COMPILER_MANIFEST_SIZE" "$DELTA_COMPILER_MANIFEST_SHA256" \
    "bootstrap/3_delta/README.md" || return $?
  require_bound_identity "delta_compiler.composed" \
    "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" \
    "$DELTA_COMPILER_COMPOSED_SIZE" "$DELTA_COMPILER_COMPOSED_SHA256" \
    "bootstrap/3_delta/README.md" || return $?
  DELTA_COMPOSED_EXPECTED="GammaComposedV1
evaluator-sha256 $GAMMA_EVALUATOR_TAPE_SHA256
source-sha256 $DELTA_COMPILER_PACKED_SHA256
source-length $DELTA_COMPILER_PACKED_SIZE"
  [ "$(cat "$OMEGA_PATH_DELTA_COMPILER_COMPOSED")" = "$DELTA_COMPOSED_EXPECTED" ] || {
    echo "bootstrap artifact: delta_compiler.composed does not bind the selected evaluator and packed closure (bootstrap/2_gamma/COMPOSED_ARTIFACT.md)" >&2
    return 3
  }
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the Delta source closure" >&2
    return 2
  }
  DELTA_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$DELTA_IDENTITY_TMP/compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE" || {
      DELTA_IDENTITY_RC=$?
      rm -rf -- "$DELTA_IDENTITY_TMP"
      return "$DELTA_IDENTITY_RC"
    }
  require_bound_identity "canonical Delta closure" \
    "$DELTA_IDENTITY_TMP/compiler.gamma" \
    "$DELTA_COMPILER_PACKED_SIZE" "$DELTA_COMPILER_PACKED_SHA256" \
    "bootstrap/3_delta/delta_compiler.composed"
  DELTA_IDENTITY_RC=$?
  rm -rf -- "$DELTA_IDENTITY_TMP"
  return "$DELTA_IDENTITY_RC"
}

# materialize_delta_compiler DEST : write the canonical entry-plus-member
# Gamma source to DEST after the bound identity check. source_closure.py
# checks every member row and writes atomically; a changed member refuses
# before DEST is written.
materialize_delta_compiler() {
  DELTA_COMPILER_DEST=$1
  require_delta_compiler_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$DELTA_COMPILER_DEST" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
}
