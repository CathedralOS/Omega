#!/usr/bin/env sh
# Focused exact OMGLOW1 -> CKIR1 two-package artifact gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "resolved-to-CKIR: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR: skipped ($TOOL absent)"
    exit 0
  }
done

LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
MUTATIONS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolved_to_ckir_mutations.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for FILE in "$LOWERER" "$RESOLVER" "$PRODUCER" "$FRAME" "$FIXTURE" "$MUTATIONS" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || { echo "resolved-to-CKIR: missing $FILE" >&2; exit 1; }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$LOWERER")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "resolved-to-CKIR: lowerer exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

"$T/lowermachine" < "$LOWERER" > "$T/lowerer.self.s"
clang -arch arm64 -o "$T/lowerer.self" "$T/lowerer.self.s"
codesign -f -s - "$T/lowerer.self" >/dev/null 2>&1

python3 "$FIXTURE" build "$T/canonical"
"$T/resolver.native" < "$T/canonical/compilation-envelope.bin" > "$T/canonical.omgrsw1"
python3 "$FRAME" pack "$T/canonical/compilation-envelope.bin" "$T/canonical.omgrsw1" > "$T/canonical.omglow"
python3 "$FRAME" verify "$T/canonical.omglow"
"$T/producer.native" < "$T/canonical/reference.bundle" > "$T/expected.ckir"

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
    echo "resolved-to-CKIR: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "resolved-to-CKIR: $LABEL published bytes on rejection" >&2
    exit 1
  fi
}

run_expect "$T/lowerer.native" "$T/canonical.omglow" 0 "$T/native.1.ckir" "native canonical"
run_expect "$T/lowerer.native" "$T/canonical.omglow" 0 "$T/native.2.ckir" "native canonical repeat"
cmp "$T/native.1.ckir" "$T/native.2.ckir" >/dev/null
cmp "$T/native.1.ckir" "$T/expected.ckir" >/dev/null
python3 "$FIXTURE" check-pair "$T/expected.ckir" "$T/native.1.ckir"

run_expect "$T/lowerer.self" "$T/canonical.omglow" 0 "$T/self.ckir" "self-built canonical"
cmp "$T/native.1.ckir" "$T/self.ckir" >/dev/null

python3 "$MUTATIONS" parameter-envelope "$T/parameter.omgc"
"$T/resolver.native" < "$T/parameter.omgc" > "$T/parameter.omgrsw1"
python3 "$FRAME" pack "$T/parameter.omgc" "$T/parameter.omgrsw1" > "$T/parameter.omglow"
run_expect "$T/lowerer.native" "$T/parameter.omglow" 0 "$T/parameter.ckir" "native parameter control"
python3 "$MUTATIONS" build "$T/canonical.omglow" "$T/parameter.omglow" "$T/mutations"

python3 - "$T/mutations/index.json" "$T/mutations.tsv" <<'PY'
import json
import sys
rows = json.load(open(sys.argv[1], encoding="utf-8"))
expected = {
    "unit-owner", "import-target-module", "binding-role",
    "declaration-reserved", "type-bool-range", "record-nominal-type",
    "field-owner", "machine-owner", "machine-parameter-owner",
    "block-body-end", "block-parameter-owner", "selected-root",
    "source-witness-body", "witness-type-count-2049",
    "omgcomp-bytes-267281", "omgrsw1-bytes-524289", "trailing-byte",
}
names = [row["name"] for row in rows]
if len(names) != len(expected) or set(names) != expected:
    raise SystemExit(f"resolved-to-CKIR mutation inventory drift: {names!r}")
with open(sys.argv[2], "w", encoding="utf-8") as output:
    for row in rows:
        output.write(f"{row['name']}\t{row['status']}\n")
PY
while IFS="	" read -r NAME EXPECTED; do
  run_expect "$T/lowerer.native" "$T/mutations/$NAME.omglow" "$EXPECTED" \
    "$T/native-$NAME.out" "native mutation $NAME"
done < "$T/mutations.tsv"

for NAME in binding-role machine-parameter-owner selected-root source-witness-body witness-type-count-2049 omgcomp-bytes-267281 omgrsw1-bytes-524289; do
  EXPECTED=251
  case "$NAME" in
    witness-type-count-2049|omgcomp-bytes-267281|omgrsw1-bytes-524289) EXPECTED=252 ;;
  esac
  run_expect "$T/lowerer.self" "$T/mutations/$NAME.omglow" "$EXPECTED" \
    "$T/self-$NAME.out" "self-built mutation $NAME"
done

echo "resolved-to-CKIR: exact two-package CKIR, phase-isolated relations, and native/self 0/251/252 agreement passed"
