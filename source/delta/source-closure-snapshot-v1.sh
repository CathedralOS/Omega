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

echo "Delta source closure V1 PASS — canonical compiler root exact; locator/cwd/symlink invariant; 251/252 no-publication"
