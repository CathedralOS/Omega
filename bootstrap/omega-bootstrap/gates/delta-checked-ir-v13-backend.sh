#!/usr/bin/env sh
# CKIR13 full-width Trapping-u32 Subtract backend gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "checked-IR-v13 backend: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3 clang codesign cmp; do command -v "$TOOL" >/dev/null 2>&1 || { echo "checked-IR-v13 backend: skipped ($TOOL absent)"; exit 0; }; done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v13-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v13_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$BACKEND" > "$T/backend.s"
clang -arch arm64 -o "$T/backend.self" "$T/backend.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1

python3 -B "$FIXTURE" emit "$T/cases"
TAB=$(printf '\t'); POSITIVE=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue; POSITIVE=$((POSITIVE+1)); CKIR="$T/cases/$NAME.ckir13"
  python3 -B "$FIXTURE" check-ir "$CKIR" "$EXPECTED"
  for IMPL in native self; do "$T/backend.$IMPL" < "$CKIR" > "$T/$NAME.$IMPL.elf"; done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" >/dev/null
  python3 -B "$FIXTURE" check-artifact "$T/$NAME.native.elf"
done < "$T/cases/positives.tsv"

for IMPL in native self; do "$T/backend.$IMPL" < "$T/cases/underflow.ckir13" > "$T/underflow.$IMPL.elf"; done
cmp "$T/underflow.native.elf" "$T/underflow.self.elf" >/dev/null
python3 -B "$FIXTURE" check-artifact "$T/underflow.native.elf"
set +e; python3 -B "$REFERENCE" run "$T/cases/underflow.ckir13" > "$T/underflow.out" 2>/dev/null; ACTUAL=$?; set -e
[ "$ACTUAL" -eq 251 ] && [ ! -s "$T/underflow.out" ] || { echo "checked-IR-v13 backend: underflow did not trap" >&2; exit 1; }

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue; COUNT=$((COUNT+1))
  for IMPL in native self; do
    set +e; "$T/backend.$IMPL" < "$T/cases/$NAME.ckir13" > "$T/$NAME.$IMPL"; ACTUAL=$?; set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.$IMPL" ] || { echo "checked-IR-v13 backend: $NAME/$IMPL failed" >&2; exit 1; }
  done
  set +e; python3 -B "$REFERENCE" validate "$T/cases/$NAME.ckir13" > "$T/$NAME.ref" 2>/dev/null; ACTUAL=$?; set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.ref" ] || { echo "checked-IR-v13 backend: $NAME/reference failed" >&2; exit 1; }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v13 backend: native/self full-u32 Const and SUB/borrow/range/store artifacts; $POSITIVE positives and $COUNT controls passed"
