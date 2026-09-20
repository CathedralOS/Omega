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

# Bound gate-local prefix entries packed on top of the bound Delta member
# closure. lowering-plan's height_driver.gamma and normalization's
# normalization_driver.gamma are single-file driver entries packed as the
# --prefix; internal-boundary and emission instead repack a gate-owned
# controls closure manifest and prefix the packed bytes, so each binds its
# manifest and the packed bytes it reproduces. The identical pins in each
# gate's {README.md,run.sh} are records of these same subjects, not
# independent identities. A digest here is an identity check on the driver or
# controls bytes; it is not a proof of the gate's judgment. Changing a driver
# or a controls member changes the packed subject and must update every
# record together.
DELTA_LOWERING_PLAN_DRIVER_SIZE=807
DELTA_LOWERING_PLAN_DRIVER_SHA256=d387d18cf6653ea2076694f15f24b81bd7078f51fba0f2b334ca0b3c731871fd
DELTA_NORMALIZATION_DRIVER_SIZE=2224
DELTA_NORMALIZATION_DRIVER_SHA256=22ebe29c49fed577e769b55e1c871638ffecabea1b156012a863a7b50df4ec90
DELTA_INTERNAL_BOUNDARY_CONTROLS_MANIFEST_SIZE=485
DELTA_INTERNAL_BOUNDARY_CONTROLS_MANIFEST_SHA256=086ba78eee677b226a09cc6afc6e0655758231fa51525bd7852ea81933ffc52e
DELTA_INTERNAL_BOUNDARY_CONTROLS_PACKED_SIZE=6127
DELTA_INTERNAL_BOUNDARY_CONTROLS_PACKED_SHA256=00e77ea3c4f86077ffdfa804e58602ca7a004b5d1cbbc94e062ff6af5614ffe9
DELTA_EMISSION_CONTROLS_MANIFEST_SIZE=951
DELTA_EMISSION_CONTROLS_MANIFEST_SHA256=8e09cd671d6908ffdd06057379f9f340e875f342bf8bd79f5099d03c2d15893d
DELTA_EMISSION_CONTROLS_PACKED_SIZE=6114
DELTA_EMISSION_CONTROLS_PACKED_SHA256=dffa334b13d250c7372fff0dd511f4f0c43cf724b9c89c16be4a2f618a11350f

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

# require_delta_lowering_plan_driver_identity : the lowering-plan gate's
# height_driver.gamma entry is the bound file. Same contract as
# require_delta_compiler_development_entry_identity.
require_delta_lowering_plan_driver_identity() {
  require_bound_identity "height_driver.gamma" \
    "$OMEGA_PATH_DELTA_LOWERING_PLAN_DRIVER" \
    "$DELTA_LOWERING_PLAN_DRIVER_SIZE" "$DELTA_LOWERING_PLAN_DRIVER_SHA256" \
    "tests/delta/lowering-plan/README.md"
}

# require_delta_normalization_driver_identity : the normalization gate's
# normalization_driver.gamma entry is the bound file. Same contract.
require_delta_normalization_driver_identity() {
  require_bound_identity "normalization_driver.gamma" \
    "$OMEGA_PATH_DELTA_NORMALIZATION_DRIVER" \
    "$DELTA_NORMALIZATION_DRIVER_SIZE" "$DELTA_NORMALIZATION_DRIVER_SHA256" \
    "tests/delta/normalization/README.md"
}

# require_delta_controls_identity LABEL SOURCES MANIFEST_SIZE MANIFEST_SHA256
#   PACKED_SIZE PACKED_SHA256 RECORD : a gate-owned controls closure's
#   manifest is the bound file and repacking it reproduces the bound packed
#   prefix bytes. Shared by the internal-boundary and emission controls.
#   Repacking needs python3; without it the check refuses rather than
#   skipping.
require_delta_controls_identity() {
  DELTA_CONTROLS_LABEL=$1
  DELTA_CONTROLS_SOURCES=$2
  DELTA_CONTROLS_MANIFEST_SIZE=$3
  DELTA_CONTROLS_MANIFEST_SHA256=$4
  DELTA_CONTROLS_PACKED_SIZE=$5
  DELTA_CONTROLS_PACKED_SHA256=$6
  DELTA_CONTROLS_RECORD=$7
  require_bound_identity "$DELTA_CONTROLS_LABEL manifest" \
    "$DELTA_CONTROLS_SOURCES" \
    "$DELTA_CONTROLS_MANIFEST_SIZE" "$DELTA_CONTROLS_MANIFEST_SHA256" \
    "$DELTA_CONTROLS_RECORD" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the $DELTA_CONTROLS_LABEL controls" >&2
    return 2
  }
  DELTA_CONTROLS_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$DELTA_CONTROLS_SOURCES" "$DELTA_CONTROLS_TMP/controls.gamma" || {
      DELTA_CONTROLS_RC=$?
      rm -rf -- "$DELTA_CONTROLS_TMP"
      return "$DELTA_CONTROLS_RC"
    }
  require_bound_identity "$DELTA_CONTROLS_LABEL packed controls" \
    "$DELTA_CONTROLS_TMP/controls.gamma" \
    "$DELTA_CONTROLS_PACKED_SIZE" "$DELTA_CONTROLS_PACKED_SHA256" \
    "$DELTA_CONTROLS_RECORD"
  DELTA_CONTROLS_RC=$?
  rm -rf -- "$DELTA_CONTROLS_TMP"
  return "$DELTA_CONTROLS_RC"
}

# require_delta_internal_boundary_controls_identity : the internal-boundary
# gate's packed controls prefix is the bound closure.
require_delta_internal_boundary_controls_identity() {
  require_delta_controls_identity "internal-boundary" \
    "$OMEGA_PATH_DELTA_INTERNAL_BOUNDARY_CONTROLS_SOURCES" \
    "$DELTA_INTERNAL_BOUNDARY_CONTROLS_MANIFEST_SIZE" \
    "$DELTA_INTERNAL_BOUNDARY_CONTROLS_MANIFEST_SHA256" \
    "$DELTA_INTERNAL_BOUNDARY_CONTROLS_PACKED_SIZE" \
    "$DELTA_INTERNAL_BOUNDARY_CONTROLS_PACKED_SHA256" \
    "tests/delta/internal-boundary/README.md"
}

# require_delta_emission_controls_identity : the emission gate's packed
# controls prefix is the bound closure.
require_delta_emission_controls_identity() {
  require_delta_controls_identity "emission" \
    "$OMEGA_PATH_DELTA_EMISSION_CONTROLS_SOURCES" \
    "$DELTA_EMISSION_CONTROLS_MANIFEST_SIZE" \
    "$DELTA_EMISSION_CONTROLS_MANIFEST_SHA256" \
    "$DELTA_EMISSION_CONTROLS_PACKED_SIZE" \
    "$DELTA_EMISSION_CONTROLS_PACKED_SHA256" \
    "tests/delta/emission/README.md"
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
