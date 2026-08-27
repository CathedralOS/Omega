#!/usr/bin/env sh
# Focused native/self gate for the bounded OMGRSW3 pure-sum relation.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "sum resolution: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "sum resolution: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "sum resolution: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/sum_resolution_fixture.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for FILE in "$RESOLVER" "$FIXTURE" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || { echo "sum resolution: missing $FILE" >&2; exit 1; }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$RESOLVER")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "sum resolution: resolver exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
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
    echo "sum resolution: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "sum resolution: $LABEL published bytes on rejection" >&2
    exit 1
  fi
}

run_expect "$T/resolver.native" "$T/fixtures/valid.omgc" 0 "$T/native-v3.witness" "native v3"
python3 "$FIXTURE" check "$T/fixtures/valid.omgc" "$T/native-v3.witness"
run_expect "$T/resolver.self" "$T/fixtures/valid.omgc" 0 "$T/self-v3.witness" "self-built v3"
cmp "$T/native-v3.witness" "$T/self-v3.witness" >/dev/null || {
  echo "sum resolution: native/self OMGRSW3 mismatch" >&2
  exit 1
}

for VERSION in v1 v2; do
  run_expect "$T/resolver.native" "$T/fixtures/legacy-$VERSION.omgc" 0 "$T/native-$VERSION.witness" "native legacy $VERSION"
  run_expect "$T/resolver.self" "$T/fixtures/legacy-$VERSION.omgc" 0 "$T/self-$VERSION.witness" "self-built legacy $VERSION"
  cmp "$T/native-$VERSION.witness" "$T/self-$VERSION.witness" >/dev/null || {
    echo "sum resolution: native/self legacy $VERSION mismatch" >&2
    exit 1
  }
  python3 "$FIXTURE" check-magic "$T/native-$VERSION.witness" "OMGRSW${VERSION#v}"
done

for NAME in mixed numbered duplicate-case duplicate-payload noncopy cycle sum-machine payloads-5-malformed; do
  run_expect "$T/resolver.native" "$T/fixtures/$NAME.omgc" 251 "$T/native-$NAME.out" "native $NAME"
done
for NAME in payloads-5 cases-65; do
  run_expect "$T/resolver.native" "$T/fixtures/$NAME.omgc" 252 "$T/native-$NAME.out" "native $NAME"
done
run_expect "$T/resolver.self" "$T/fixtures/cycle.omgc" 251 "$T/self-cycle.out" "self-built cycle"
run_expect "$T/resolver.self" "$T/fixtures/payloads-5.omgc" 252 "$T/self-payloads-5.out" "self-built payloads-5"

echo "sum resolution: OMGRSW3 native/self, legacy least-version, 251, and 252 controls passed"
