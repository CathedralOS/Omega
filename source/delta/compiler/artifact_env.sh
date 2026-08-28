#!/usr/bin/env sh
# Loader for the installed, reconstruction-verified Darwin ARM64 Delta compiler.
# Source tools/lattice/paths.sh first. This loader never rebuilds or substitutes
# an ambient compiler when the admitted artifact is absent.

[ -n "${OMEGA_PATH_DELTA_COMPILER:-}" ] || {
  echo "delta artifact: source tools/lattice/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

DELTA_COMPILER_ARTIFACT_ROOT=$OMEGA_PATH_DELTA_COMPILER/artifacts/darwin-arm64-v1
DELTA_COMPILER_ARTIFACT=$DELTA_COMPILER_ARTIFACT_ROOT/delta-compiler
DELTA_COMPILER_ASSEMBLY_RECEIPT=$DELTA_COMPILER_ARTIFACT_ROOT/assembly-publication-receipt.json
DELTA_COMPILER_REALIZATION_OBSERVATION=$DELTA_COMPILER_ARTIFACT_ROOT/realization-observation.json
DELTA_COMPILER_ARTIFACT_CUSTODY_RECEIPT=$DELTA_COMPILER_ARTIFACT_ROOT/artifact-custody-receipt.json
DELTA_COMPILER_EXECUTION_RAW=$DELTA_COMPILER_ARTIFACT_ROOT/execution.raw
DELTA_COMPILER_INSTALLATION_MANIFEST=$DELTA_COMPILER_ARTIFACT_ROOT/installation.json

export DELTA_COMPILER_ARTIFACT_ROOT DELTA_COMPILER_ARTIFACT
export DELTA_COMPILER_ASSEMBLY_RECEIPT DELTA_COMPILER_REALIZATION_OBSERVATION
export DELTA_COMPILER_ARTIFACT_CUSTODY_RECEIPT DELTA_COMPILER_EXECUTION_RAW
export DELTA_COMPILER_INSTALLATION_MANIFEST

require_delta_compiler_artifact() {
  [ -x "$DELTA_COMPILER_ARTIFACT" ] || {
    echo "delta artifact: missing $DELTA_COMPILER_ARTIFACT" >&2
    return 2
  }
  for path in "$DELTA_COMPILER_ASSEMBLY_RECEIPT" \
    "$DELTA_COMPILER_REALIZATION_OBSERVATION" \
    "$DELTA_COMPILER_ARTIFACT_CUSTODY_RECEIPT" \
    "$DELTA_COMPILER_EXECUTION_RAW" "$DELTA_COMPILER_INSTALLATION_MANIFEST"; do
    [ -f "$path" ] || {
      echo "delta artifact: missing $path" >&2
      return 2
    }
  done
}
