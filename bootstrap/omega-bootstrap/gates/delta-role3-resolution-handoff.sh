#!/usr/bin/env sh
# Focused native/self gate for ordinary attached self-call OMGRSW1 role-3 rows.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "role-3 resolution: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "role-3 resolution: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "role-3 resolution: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for FILE in "$RESOLVER" "$FIXTURE" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || { echo "role-3 resolution: missing $FILE" >&2; exit 1; }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$RESOLVER")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "role-3 resolution: resolver exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$RESOLVER" > "$T/resolver.self.s"
clang -arch arm64 -o "$T/resolver.self" "$T/resolver.self.s"
codesign -f -s - "$T/resolver.self" >/dev/null 2>&1

python3 "$FIXTURE" build "$T/fixtures"

run_expect() {
  EXE=$1
  INPUT=$2
  EXPECTED=$3
  OUTPUT=$4
  LABEL=$5
  set +e
  "$EXE" < "$INPUT" > "$OUTPUT" 2> "$OUTPUT.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "role-3 resolution: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "role-3 resolution: $LABEL published bytes on rejection" >&2
    exit 1
  fi
}

run_expect "$T/resolver.native" "$T/fixtures/valid.omgc" 0 "$T/native.witness" "native valid"
python3 "$FIXTURE" check "$T/fixtures/valid.omgc" "$T/native.witness"
run_expect "$T/resolver.self" "$T/fixtures/valid.omgc" 0 "$T/self.witness" "self-built valid"
cmp "$T/native.witness" "$T/self.witness" >/dev/null || {
  echo "role-3 resolution: native/self witness mismatch" >&2
  exit 1
}

for NAME in missing wrong-owner private-cross-module; do
  run_expect "$T/resolver.native" "$T/fixtures/$NAME.omgc" 251 "$T/native-$NAME.out" "native $NAME"
done
run_expect "$T/resolver.self" "$T/fixtures/missing.omgc" 251 "$T/self-missing.out" "self-built missing"

echo "role-3 resolution: native/self exact binding and focused 251 controls passed"
