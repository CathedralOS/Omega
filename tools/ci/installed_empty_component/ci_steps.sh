#!/usr/bin/env bash
# Reviewed proposal only. The caller supplies fresh final-candidate paths.
# Wrap this entire script in one tools/build_slot.py hold on a POSIX CI host.
set -euo pipefail
: "${REPOSITORY_ROOT:?supply the clean CI checkout}"
: "${EXPECTED_COMPILER_SOURCE:?supply the reviewed final 40-character compiler source}"
: "${COMPILER_INPUT_ROSTER:?supply the independently reviewed ten-object manifest}"
: "${COMPILER_INPUT_ROSTER_SHA256:?supply its pinned SHA256; never derive the roster from HEAD}"
: "${PRODUCER_PREPARATION:?supply actual successful producer preparation.json}"
: "${FROZEN_RESULTS:?supply actual full98 observations.json}"
: "${FROZEN_INPUT_ROOT:?supply the restored frozen source root authenticated by CAPTURE_BINDING}"
: "${RESTORED_PRODUCER_IMAGE:?supply the captured CLI restored under CAPTURE_ROOT}"
: "${CANONICAL_ARTIFACT:?supply the exact installed-empty inspect artifact}"
: "${CAPTURE_BINDING:?supply the independently reviewed actual capture identity}"
: "${CAPTURE_ROOT:?supply the actual restored exported capture root}"
: "${CAPTURE_BINDING_SHA256:?supply its pinned SHA256}"
: "${COMPONENT_OUTPUT:?supply a new ignored output directory}"
driver="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/qualify_installed_empty.py"
cd -- "$REPOSITORY_ROOT"
python "$driver" prepare --repository "$REPOSITORY_ROOT" \
  --expected-compiler-source "$EXPECTED_COMPILER_SOURCE" \
  --compiler-input-roster "$COMPILER_INPUT_ROSTER" \
  --compiler-input-roster-sha256 "$COMPILER_INPUT_ROSTER_SHA256" \
  --out "$COMPONENT_OUTPUT/codec-image"
cli="$RESTORED_PRODUCER_IMAGE"
image="$(python -c 'import json,sys; print(json.load(open(sys.argv[1]))["retained_image"])' "$COMPONENT_OUTPUT/codec-image/preparation.json")"
python "$driver" qualify --repository "$REPOSITORY_ROOT" \
  --expected-compiler-source "$EXPECTED_COMPILER_SOURCE" \
  --compiler-input-roster "$COMPILER_INPUT_ROSTER" \
  --compiler-input-roster-sha256 "$COMPILER_INPUT_ROSTER_SHA256" \
  --capture-binding "$CAPTURE_BINDING" --capture-binding-sha256 "$CAPTURE_BINDING_SHA256" \
  --capture-root "$CAPTURE_ROOT" \
  --preparation "$PRODUCER_PREPARATION" --results "$FROZEN_RESULTS" \
  --producer-image "$cli" --receiver-image "$cli" \
  --source "$FROZEN_INPUT_ROOT/build/supplied-output-loan/installed-empty-snapshot-control/main.omg" \
  --build "$FROZEN_INPUT_ROOT/build/supplied-output-loan/installed-empty-snapshot-control/build.omg" \
  --artifact "$CANONICAL_ARTIFACT" \
  --test-preparation "$COMPONENT_OUTPUT/codec-image/preparation.json" \
  --test-image "$image" --out "$COMPONENT_OUTPUT/omission"
