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
# is the ordered source authority, support/support.gamma.sources is the ordered
# runtime/adapter member authority, and delta_compiler.composed binds the packed
# entry-plus-member bytes, the packed support bytes, and the selected evaluator
# tape under GammaComposedV2 (bootstrap/2_gamma/COMPOSED_ARTIFACT.md). The packed
# closure identity is the same source-sha256/source-length the composed record
# and the staged-compiler and normalization compiler.tsv rows pin; the packed
# support identity is the same support-sha256/support-length the record pins.
# A digest here is an identity check
# that the bytes being materialized are the bound ones; it is not a proof of
# the compiler. Changing any member, the entry, either manifest, or the packed
# support section invalidates the dependent evidence and must update every
# record.
DELTA_COMPILER_ENTRY_SIZE=813
DELTA_COMPILER_ENTRY_SHA256=f12836610a7d8cb7da7f1288c20d870423cde4497aa8d1a2cf2962e8b24a20f9
DELTA_COMPILER_MANIFEST_SIZE=11137
DELTA_COMPILER_MANIFEST_SHA256=653486437acef2b97fdd0e54a28a77af68fe77485a6816248928b547b7a48bf3
DELTA_COMPILER_COMPOSED_SIZE=298
DELTA_COMPILER_COMPOSED_SHA256=11c86871fc05f2739eaa7a6a4ba58c2e99edcae71e9af2bf1723498effe83965
DELTA_COMPILER_PACKED_SIZE=147840
DELTA_COMPILER_PACKED_SHA256=fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929
DELTA_COMPILER_SUPPORT_MANIFEST_SIZE=490
DELTA_COMPILER_SUPPORT_MANIFEST_SHA256=cf20f4a6331c3af516dbed8bc206298d1d1205b4fb4801d025ad4256a5a6d9f3
DELTA_COMPILER_SUPPORT_PACKED_SIZE=2998
DELTA_COMPILER_SUPPORT_PACKED_SHA256=cfdf07cf8010eba2fd7da47e6936ea1e237f637f4ded5791c272e03096d70255

# Bound diagnostic prefix. The staged-compiler gate's
# tests/delta/staged-compiler/development_driver.gamma is a separate entry
# packed on top of the bound member closure as an unmarked raw-source
# transformer; it never selects a compiler application profile, so the packed
# development compiler is a diagnostic subject, not a second canonical
# compiler. The identical pins in
# tests/delta/staged-compiler/{README.md,run.sh} and
# tests/bootstrap/source-closure.py are records of this one subject, not
# independent identities. A digest here is an identity check on the entry
# bytes; it is not a proof of the staged pipeline. Changing the driver changes
# the packed development compiler and must update every record together.
DELTA_COMPILER_DEVELOPMENT_ENTRY_SIZE=580
DELTA_COMPILER_DEVELOPMENT_ENTRY_SHA256=7bcf4098ff44fb5cec57659b7d3c1ceddfbb50a05e1e9f2ec9704be0da5b95bb

# require_delta_compiler_identity : the canonical entry, manifests, and
# composed record are the bound files; the composed record names the selected
# Gamma evaluator, packed closure, and packed support section; and repacking
# each manifest reproduces exactly the bound packed bytes. Every
# materialization runs it; tests may call it
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
  require_bound_identity "support.gamma.sources" \
    "$OMEGA_PATH_DELTA_COMPILER_SUPPORT_SOURCES" \
    "$DELTA_COMPILER_SUPPORT_MANIFEST_SIZE" \
    "$DELTA_COMPILER_SUPPORT_MANIFEST_SHA256" \
    "bootstrap/3_delta/README.md" || return $?
  require_bound_identity "delta_compiler.composed" \
    "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" \
    "$DELTA_COMPILER_COMPOSED_SIZE" "$DELTA_COMPILER_COMPOSED_SHA256" \
    "bootstrap/3_delta/README.md" || return $?
  DELTA_COMPOSED_EXPECTED="GammaComposedV2
evaluator-sha256 $GAMMA_EVALUATOR_TAPE_SHA256
source-sha256 $DELTA_COMPILER_PACKED_SHA256
source-length $DELTA_COMPILER_PACKED_SIZE
support-sha256 $DELTA_COMPILER_SUPPORT_PACKED_SHA256
support-length $DELTA_COMPILER_SUPPORT_PACKED_SIZE"
  [ "$(cat "$OMEGA_PATH_DELTA_COMPILER_COMPOSED")" = "$DELTA_COMPOSED_EXPECTED" ] || {
    echo "bootstrap artifact: delta_compiler.composed does not bind the selected evaluator, packed closure, and packed support section (bootstrap/2_gamma/COMPOSED_ARTIFACT.md)" >&2
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
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SUPPORT_SOURCES" \
    "$DELTA_IDENTITY_TMP/support.bin" || {
      DELTA_IDENTITY_RC=$?
      rm -rf -- "$DELTA_IDENTITY_TMP"
      return "$DELTA_IDENTITY_RC"
    }
  require_bound_identity "canonical Delta closure" \
    "$DELTA_IDENTITY_TMP/compiler.gamma" \
    "$DELTA_COMPILER_PACKED_SIZE" "$DELTA_COMPILER_PACKED_SHA256" \
    "bootstrap/3_delta/delta_compiler.composed" || {
      DELTA_IDENTITY_RC=$?
      rm -rf -- "$DELTA_IDENTITY_TMP"
      return "$DELTA_IDENTITY_RC"
    }
  require_bound_identity "canonical Delta support section" \
    "$DELTA_IDENTITY_TMP/support.bin" \
    "$DELTA_COMPILER_SUPPORT_PACKED_SIZE" \
    "$DELTA_COMPILER_SUPPORT_PACKED_SHA256" \
    "bootstrap/3_delta/delta_compiler.composed"
  DELTA_IDENTITY_RC=$?
  rm -rf -- "$DELTA_IDENTITY_TMP"
  return "$DELTA_IDENTITY_RC"
}

# require_delta_compiler_development_entry_identity : the diagnostic entry the
# staged-compiler gate prefixes onto the bound member closure is the bound
# file. The driver is a separate input from the canonical entry and never part
# of the sealed DCREQ subject, so consumers that pack it run this before
# packing; tests may call it directly. It is not part of the canonical closure
# and does not run during materialization.
require_delta_compiler_development_entry_identity() {
  require_bound_identity "development_driver.gamma" \
    "$OMEGA_PATH_DELTA_COMPILER_DEVELOPMENT_ENTRY" \
    "$DELTA_COMPILER_DEVELOPMENT_ENTRY_SIZE" \
    "$DELTA_COMPILER_DEVELOPMENT_ENTRY_SHA256" \
    "tests/delta/staged-compiler/README.md"
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

# materialize_delta_support DEST : write the packed bound support section to
# DEST after the same bound identity check. The sealed input of every Delta
# compiler invocation is the edge payload followed by these exact bytes.
materialize_delta_support() {
  DELTA_SUPPORT_DEST=$1
  require_delta_compiler_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SUPPORT_SOURCES" "$DELTA_SUPPORT_DEST"
}
