#!/usr/bin/env sh
# Focused gate for the standalone exact OMGCOMP -> canonical OMGRSW1 resolver.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "resolution handoff: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolution handoff: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolution handoff: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
NEGATIVES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_negatives.py"
BUNDLE="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for FILE in "$RESOLVER" "$PRODUCER" "$REFERENCE" "$FIXTURE" "$NEGATIVES" "$BUNDLE" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || { echo "resolution handoff: missing $FILE" >&2; exit 1; }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$RESOLVER")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "resolution handoff: resolver exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

if ! "$T/lowermachine" < "$RESOLVER" > "$T/resolver.self.s"; then
  echo "resolution handoff: Delta-self compiler could not lower resolver" >&2
  exit 1
fi
clang -arch arm64 -o "$T/resolver.self" "$T/resolver.self.s"
codesign -f -s - "$T/resolver.self" >/dev/null 2>&1

python3 "$FIXTURE" build "$T/canonical"
python3 "$NEGATIVES" build "$T/negatives"
python3 "$NEGATIVES" check "$T/negatives"
python3 "$REFERENCE" build-controls "$T/controls"
python3 - "$T/negatives/index.json" <<'PY'
import json
import sys
index = json.load(open(sys.argv[1], encoding="utf-8"))
if len(index.get("cases", ())) != 10:
    raise SystemExit("resolution handoff: expected exact 10-case semantic-negative inventory")
PY

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
    echo "resolution handoff: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "resolution handoff: $LABEL published bytes on rejection" >&2
    exit 1
  fi
}

CANONICAL="$T/canonical/compilation-envelope.bin"
run_expect "$T/resolver.native" "$CANONICAL" 0 "$T/canonical.native.1" "native canonical"
run_expect "$T/resolver.native" "$CANONICAL" 0 "$T/canonical.native.2" "native canonical repeat"
cmp "$T/canonical.native.1" "$T/canonical.native.2" >/dev/null || {
  echo "resolution handoff: native output is nondeterministic" >&2
  exit 1
}
python3 "$REFERENCE" check-canonical "$CANONICAL" "$T/canonical.native.1"

for ENVELOPE in "$T/negatives"/*/compilation-envelope.bin; do
  NAME=$(basename "$(dirname "$ENVELOPE")")
  run_expect "$T/resolver.native" "$ENVELOPE" 251 "$T/negative-$NAME.out" "native semantic $NAME"
done

python3 - "$T/controls/index.json" "$T/controls.tsv" <<'PY'
import json
import sys
rows = json.load(open(sys.argv[1], encoding="utf-8"))
with open(sys.argv[2], "w", encoding="utf-8") as output:
    for row in rows:
        output.write(f"{row['name']}\t{row['status']}\n")
PY
while IFS="	" read -r NAME EXPECTED; do
  OUTPUT="$T/control-$NAME.out"
  run_expect "$T/resolver.native" "$T/controls/$NAME.omgc" "$EXPECTED" "$OUTPUT" "native control $NAME"
  if [ "$EXPECTED" -eq 0 ]; then
    python3 "$REFERENCE" check-control "$NAME" "$OUTPUT"
  fi
done < "$T/controls.tsv"

# The frozen one-unit producer is the executable oracle for byte-exact CKIR1
# type rows. Its source parser admits interleaved one-level arrays; the resolver
# itself additionally exercises nested arrays through its repeated raw pass.
python3 "$BUNDLE" pack main.omg="$T/controls/array-order.omg" > "$T/array.bundle"
run_expect "$T/producer.native" "$T/array.bundle" 0 "$T/array.ckir" "frozen array oracle"
python3 "$REFERENCE" check-type-parity "$T/control-array-order.out" "$T/array.ckir"

run_expect "$T/resolver.self" "$CANONICAL" 0 "$T/canonical.self" "self-built canonical"
cmp "$T/canonical.native.1" "$T/canonical.self" >/dev/null || {
  echo "resolution handoff: native/self canonical witnesses differ" >&2
  exit 1
}
run_expect "$T/resolver.self" "$T/controls/parameter-spans.omgc" 0 "$T/parameter-spans.self" "self-built parameter spans"
cmp "$T/control-parameter-spans.out" "$T/parameter-spans.self" >/dev/null || {
  echo "resolution handoff: native/self parameter-span witnesses differ" >&2
  exit 1
}
python3 "$REFERENCE" check-control parameter-spans "$T/parameter-spans.self"
run_expect "$T/resolver.self" "$T/negatives/private-import/compilation-envelope.bin" 251 "$T/self-251.out" "self-built semantic rejection"
run_expect "$T/resolver.self" "$T/controls/imports-65.omgc" 252 "$T/self-252.out" "self-built resource exhaustion"

echo "resolution handoff: native matrix and Delta-self agreement passed"
