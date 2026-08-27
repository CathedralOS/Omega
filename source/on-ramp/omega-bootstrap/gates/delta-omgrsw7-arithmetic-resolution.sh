#!/usr/bin/env sh
# Focused native/self OMGRSW7 least-selection and full-u32 custody gate.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRSW7 resolution: skipped (requires Darwin arm64)"; exit 0 ;;
esac

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/omgrsw7_arithmetic_resolution_fixture.py"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
[ -x "$DELTA" ] || cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$RESOLVER" > "$T/resolver.self.s"
clang -arch arm64 -o "$T/resolver.self" "$T/resolver.self.s"
codesign -f -s - "$T/resolver.self" >/dev/null 2>&1
python3 "$FIXTURE" build "$T/fixtures"

python3 - "$T/fixtures/index.json" <<'PY' | while IFS="	" read -r NAME STATUS MAGIC; do
import json, sys
for row in json.load(open(sys.argv[1], encoding="utf-8")):
    print(row["name"], row["status"], row["magic"] or "-", sep="\t")
PY
  for MODE in native self; do
    set +e
    "$T/resolver.$MODE" < "$T/fixtures/$NAME.omgc" > "$T/$MODE-$NAME.out"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$STATUS" ] || {
      echo "OMGRSW7 resolution: $MODE $NAME returned $ACTUAL, expected $STATUS" >&2
      exit 1
    }
    if [ "$STATUS" -ne 0 ]; then
      [ ! -s "$T/$MODE-$NAME.out" ] || { echo "OMGRSW7 resolution: rejection published output" >&2; exit 1; }
    else
      python3 "$FIXTURE" magic "$T/$MODE-$NAME.out" "$MAGIC"
    fi
  done
  [ "$STATUS" -ne 0 ] || cmp "$T/native-$NAME.out" "$T/self-$NAME.out" >/dev/null
done

python3 "$FIXTURE" check "$T/native-maximum-literal.out"
python3 "$FIXTURE" check-links "$T/native-all-named-leaves.out"
python3 "$FIXTURE" check-view "$T/native-view-plus-arithmetic.out"
echo "OMGRSW7 resolution: native/self least selection, exclusions, and full-u32 custody passed"
