#!/usr/bin/env sh
# CKIR15 recurrent shared-view conservative backend gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v15 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v15 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v15-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v15_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
V14_GATE="$GATE_DIR/delta-checked-ir-v14-backend.sh"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
python3 -B "$FIXTURE" run-filter "$T/lowermachine" "$BACKEND" \
  "$T/backend.s" 0 nonempty
clang -arch arm64 -o "$T/backend.self" "$T/backend.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1

python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/positives.tsv" "$T/repeat/positives.tsv"
cmp "$T/cases/manifest.tsv" "$T/repeat/manifest.tsv"

TAB=$(printf '\t')
POSITIVE=0
while IFS="$TAB" read -r NAME OUTCOME; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  CKIR="$T/cases/$NAME.ckir15"
  cmp "$CKIR" "$T/repeat/$NAME.ckir15" || {
    echo "checked-IR-v15 backend: nondeterministic $NAME carrier" >&2; exit 1;
  }
  python3 -B "$FIXTURE" check-ir "$CKIR" "$OUTCOME"
  python3 -B "$REFERENCE" validate "$CKIR" >/dev/null
  [ "$(python3 -B "$REFERENCE" run "$CKIR")" = "$OUTCOME" ] || {
    echo "checked-IR-v15 backend: $NAME runtime meaning mismatch" >&2; exit 1;
  }
  if [ "$OUTCOME" = library ]; then
    POLICY=empty
  else
    POLICY=nonempty
  fi
  for IMPLEMENTATION in native self; do
    python3 -B "$FIXTURE" run-filter "$T/backend.$IMPLEMENTATION" "$CKIR" \
      "$T/$NAME.$IMPLEMENTATION.elf" 0 "$POLICY"
    if [ "$OUTCOME" != library ]; then
      python3 -B "$FIXTURE" check-artifact \
        "$T/$NAME.$IMPLEMENTATION.elf" "$CKIR"
    fi
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v15 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
done < "$T/cases/positives.tsv"

# Exercise the actual resolver/lowerer cross-pair as well as handcrafted rows.
# This carrier deliberately retains the inherited bounded plain-u32 canonical
# row while using a separate exact full-width type only when arithmetic needs it.
python3 - "$FIXTURE" "$T/produced" <<'PY'
import subprocess
import sys
subprocess.run(
    [sys.executable, "-B", sys.argv[1], "emit-produced", sys.argv[2]],
    check=True,
    timeout=20,
)
PY
PRODUCED_CKIR="$T/produced/produced-two-byte.ckir15"
python3 -B "$FIXTURE" check-produced-ir "$PRODUCED_CKIR"
python3 -B "$REFERENCE" validate "$PRODUCED_CKIR" >/dev/null
[ "$(python3 -B "$REFERENCE" run "$PRODUCED_CKIR")" = 70 ] || {
  echo "checked-IR-v15 backend: produced two-byte meaning mismatch" >&2; exit 1;
}
for IMPLEMENTATION in native self; do
  python3 -B "$FIXTURE" run-filter "$T/backend.$IMPLEMENTATION" \
    "$PRODUCED_CKIR" "$T/produced-two-byte.$IMPLEMENTATION.elf" 0 nonempty
  python3 -B "$FIXTURE" check-artifact \
    "$T/produced-two-byte.$IMPLEMENTATION.elf" "$PRODUCED_CKIR"
done
cmp "$T/produced-two-byte.native.elf" "$T/produced-two-byte.self.elf" || {
  echo "checked-IR-v15 backend: produced two-byte native/self artifact mismatch" >&2
  exit 1
}

# The recognizer must observe both independently emitted partial templates.
python3 -B "$FIXTURE" mutate-second-head \
  "$T/two-byte-recurrent.native.elf" "$T/bad-second-head.elf"
if python3 -B "$FIXTURE" check-artifact "$T/bad-second-head.elf" \
  "$T/cases/two-byte-recurrent.ckir15" >/dev/null 2>&1; then
  echo "checked-IR-v15 backend: mutated second head template accepted" >&2
  exit 1
fi

INVALID=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  INVALID=$((INVALID + 1))
  CKIR="$T/cases/$NAME.ckir15"
  cmp "$CKIR" "$T/repeat/$NAME.ckir15" || {
    echo "checked-IR-v15 backend: nondeterministic $NAME control" >&2; exit 1;
  }
  for IMPLEMENTATION in native self; do
    python3 -B "$FIXTURE" run-filter "$T/backend.$IMPLEMENTATION" "$CKIR" \
      "$T/$NAME.$IMPLEMENTATION.out" "$EXPECTED" empty
  done
  set +e
  python3 -B "$REFERENCE" validate "$CKIR" \
    > "$T/$NAME.reference.out" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference.out" ] || {
    echo "checked-IR-v15 backend: $NAME/reference returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
done < "$T/cases/manifest.tsv"

# Keep both inherited backend contracts executable; CKIR14 invokes CKIR12.
sh "$V14_GATE"

echo "checked-IR-v15 backend: $POSITIVE deterministic handcrafted plus one real producer/lowerer cross-pair preserve recurrent/one-byte/empty/runtime-origin/optional-arithmetic templates and native/self identity; $INVALID vector/identity/resource controls passed; CKIR14 and CKIR12 regressions passed"
