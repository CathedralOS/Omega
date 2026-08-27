#!/usr/bin/env sh
# Path-independent Delta source-closure V1 reference and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta source closure V1: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

VERIFY=$OMEGA_PATH_DELTA/source_closure_snapshot_v1.py
SNAPSHOT=$OMEGA_PATH_DELTA/source-closures/canonical-compiler-v1.json
LOCATIONS=$OMEGA_PATH_DELTA/source-closures/canonical-compiler-v1.locations.json
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT HUP INT TERM

python3 -B "$VERIFY" verify "$SNAPSHOT" "$LOCATIONS" "delta=$OMEGA_PATH_DELTA" > "$T/canonical.out"
python3 -B "$VERIFY" mutations "$SNAPSHOT" "$LOCATIONS" "delta=$OMEGA_PATH_DELTA" > "$T/mutations.out"

# The same immutable semantic snapshot must validate after a physical rename,
# from an unrelated cwd, and through an equivalent symlink locator.  Only the
# uncommitted diagnostic sidecar changes.
mkdir -p "$T/relocated/renamed" "$T/relocated/alias"
cp "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/relocated/renamed/compiler-source.bytes"
ln -s ../renamed/compiler-source.bytes "$T/relocated/alias/compiler-source-link"

write_locations() { # relative-path output
  python3 -B - "$1" "$2" <<'PY'
import json
import sys
value = {
    "artifacts": [],
    "schema": "omega.delta-source-closure-locations.v1",
    "snapshot_id": "delta.compiler.current.v1",
    "sources": [{
        "id": "delta.compiler.lowermachine",
        "relative_path": sys.argv[1],
        "repository_role": "relocated",
    }],
    "tool_artifacts": [],
}
with open(sys.argv[2], "w", encoding="utf-8") as stream:
    json.dump(value, stream, indent=2, sort_keys=True)
    stream.write("\n")
PY
}

write_locations renamed/compiler-source.bytes "$T/relocated.locations.json"
write_locations alias/compiler-source-link "$T/symlink.locations.json"
(
  cd "$T"
  python3 -B "$VERIFY" verify "$SNAPSHOT" "$T/relocated.locations.json" "relocated=$T/relocated" > "$T/relocated.out"
  python3 -B "$VERIFY" verify "$SNAPSHOT" "$T/symlink.locations.json" "relocated=$T/relocated" > "$T/symlink.out"
)
cmp "$T/canonical.out" "$T/relocated.out" >/dev/null
cmp "$T/canonical.out" "$T/symlink.out" >/dev/null

expect_reject() { # expected-status name manifest locations role
  EXPECTED=$1 NAME=$2 CANDIDATE=$3 CANDIDATE_LOCATIONS=$4 ROLE=$5
  set +e
  python3 -B "$VERIFY" verify "$CANDIDATE" "$CANDIDATE_LOCATIONS" "$ROLE" > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "Delta source closure V1: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$NAME.out" ] || {
    echo "Delta source closure V1: $NAME published stdout on rejection" >&2
    exit 1
  }
}

# Locator spellings never create source identity: changed bytes under a valid
# diagnostic path reject against the path-independent digest.
cp "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/relocated/renamed/wrong.bytes"
printf '\000' >> "$T/relocated/renamed/wrong.bytes"
write_locations renamed/wrong.bytes "$T/wrong.locations.json"
expect_reject 251 wrong-content "$SNAPSHOT" "$T/wrong.locations.json" "relocated=$T/relocated"

# Document ceilings select resource status before JSON inspection and publish
# no stdout bytes.
python3 -B - "$T/oversize.json" <<'PY'
import sys
with open(sys.argv[1], "wb") as stream:
    stream.write(b" " * 65537)
PY
expect_reject 252 manifest-ceiling "$T/oversize.json" "$LOCATIONS" "delta=$OMEGA_PATH_DELTA"

# Do not silently publish a two-root substitute after the reviewed focused
# lowerer arrives.  The complete provisional profile is three roots or absent.
BRIDGE_RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-u64-buffer-resolve.alp
BRIDGE_LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-u64-buffer-to-ckir.alp
BRIDGE_BACKEND=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v18-to-elf.alp
BRIDGE_SNAPSHOT=$OMEGA_PATH_DELTA/source-closures/u64-buffer-bridge-slice-v1.json
BRIDGE_LOCATIONS=$OMEGA_PATH_DELTA/source-closures/u64-buffer-bridge-slice-v1.locations.json
if [ -f "$BRIDGE_RESOLVER" ] && [ -f "$BRIDGE_LOWERER" ] && [ -f "$BRIDGE_BACKEND" ]; then
  [ -f "$BRIDGE_SNAPSHOT" ] && [ -f "$BRIDGE_LOCATIONS" ] || {
    echo "Delta source closure V1: focused three-root bridge landed without its provisional snapshot" >&2
    exit 1
  }
  case "$(uname -sm)" in
    "Darwin arm64") BRIDGE_HOST=1 ;;
    *) BRIDGE_HOST=0 ;;
  esac
  for TOOL in cargo clang codesign cmp python3; do
    command -v "$TOOL" >/dev/null 2>&1 || BRIDGE_HOST=0
  done
  if [ "$BRIDGE_HOST" -eq 1 ]; then
    BRIDGE_GENERATED=$T/bridge
    mkdir -p "$BRIDGE_GENERATED/source" "$BRIDGE_GENERATED/ckir-reference"
    SOURCE_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/omgrsw10_u64_buffer_fixture.py
    LOWERING_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-u64-buffer-to-ckir18-fixture.py
    CKIR_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v18-fixture.py
    BACKEND_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v18-backend-fixture.py

    python3 -B "$SOURCE_FIXTURE" build "$BRIDGE_GENERATED/source"
    python3 -B "$CKIR_FIXTURE" emit "$BRIDGE_GENERATED/ckir-reference"
    cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
    DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
    env DELTA_ARCH=aarch64 "$DELTA" "$OMEGA_PATH_DELTA/samples/lowermachine.alp" \
      "$BRIDGE_GENERATED/lowermachine.signed" >/dev/null
    cp "$BRIDGE_GENERATED/lowermachine.signed" "$BRIDGE_GENERATED/lowermachine.unsigned"
    codesign --remove-signature "$BRIDGE_GENERATED/lowermachine.unsigned"
    cp "$OMEGA_PATH_DELTA/source-closures/tool-manifests/delta-compiler-darwin-arm64-v1.json" \
      "$BRIDGE_GENERATED/delta-compiler.manifest.json"

    build_root() { # source stem
      SOURCE=$1 STEM=$2
      python3 -B - "$SOURCE" "$BRIDGE_GENERATED/$STEM.translation-unit" <<'PY'
from pathlib import Path
import sys
source, output = map(Path, sys.argv[1:])
output.write_bytes(source.read_bytes() + b"\n")
PY
      "$BRIDGE_GENERATED/lowermachine.signed" \
        < "$BRIDGE_GENERATED/$STEM.translation-unit" \
        > "$BRIDGE_GENERATED/$STEM.s"
      clang -arch arm64 -Wl,-no_uuid -o "$BRIDGE_GENERATED/$STEM.signed" \
        "$BRIDGE_GENERATED/$STEM.s"
      codesign -f -s - "$BRIDGE_GENERATED/$STEM.signed" >/dev/null 2>&1
      cp "$BRIDGE_GENERATED/$STEM.signed" "$BRIDGE_GENERATED/$STEM.unsigned"
      codesign --remove-signature "$BRIDGE_GENERATED/$STEM.unsigned"
    }
    build_root "$BRIDGE_RESOLVER" resolver
    build_root "$BRIDGE_LOWERER" lowerer
    build_root "$BRIDGE_BACKEND" backend

    cp "$BRIDGE_GENERATED/source/canonical.omgc" "$BRIDGE_GENERATED/canonical.omgcomp1"
    "$BRIDGE_GENERATED/resolver.signed" \
      < "$BRIDGE_GENERATED/canonical.omgcomp1" \
      > "$BRIDGE_GENERATED/canonical.omgrswa10"
    cmp "$BRIDGE_GENERATED/canonical.omgrswa10" \
      "$BRIDGE_GENERATED/source/canonical.omgrswa" >/dev/null
    python3 -B "$LOWERING_FIXTURE" pack \
      "$BRIDGE_GENERATED/canonical.omgcomp1" \
      "$BRIDGE_GENERATED/canonical.omgrswa10" \
      "$BRIDGE_GENERATED/canonical.omglowj19"
    "$BRIDGE_GENERATED/lowerer.signed" \
      < "$BRIDGE_GENERATED/canonical.omglowj19" \
      > "$BRIDGE_GENERATED/canonical.ckir18"
    cmp "$BRIDGE_GENERATED/canonical.ckir18" \
      "$BRIDGE_GENERATED/ckir-reference/canonical.ckir18" >/dev/null
    "$BRIDGE_GENERATED/backend.signed" \
      < "$BRIDGE_GENERATED/canonical.ckir18" \
      > "$BRIDGE_GENERATED/canonical.elf"
    python3 -B "$BACKEND_FIXTURE" check-artifact \
      "$BRIDGE_GENERATED/canonical.elf" "$BRIDGE_GENERATED/canonical.ckir18"

    python3 -B "$VERIFY" verify "$BRIDGE_SNAPSHOT" "$BRIDGE_LOCATIONS" \
      "omega-bootstrap-compiler=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" \
      "generated=$BRIDGE_GENERATED" >/dev/null
    python3 -B "$VERIFY" mutations "$BRIDGE_SNAPSHOT" "$BRIDGE_LOCATIONS" \
      "omega-bootstrap-compiler=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" \
      "generated=$BRIDGE_GENERATED" >/dev/null
    BRIDGE_RESULT="three-root provisional bridge source/tool/action DAG exact"
  else
    BRIDGE_RESULT="three-root provisional bridge artifact replay skipped (requires Darwin arm64 toolchain)"
  fi
else
  BRIDGE_RESULT="three-root provisional bridge snapshot deferred (focused lowerer absent)"
fi

echo "Delta source closure V1 PASS — canonical root exact; locator/cwd/symlink invariant; 251/252 no-publication; $BRIDGE_RESULT"
