#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "checked-IR-v16 backend: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3 clang codesign cmp; do command -v "$TOOL" >/dev/null 2>&1 || { echo "checked-IR-v16 backend: skipped ($TOOL absent)"; exit 0; }; done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v16-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v16_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
python3 -B "$FIXTURE" run-filter "$T/lowermachine" "$BACKEND" "$T/backend.s" 0 nonempty
clang -arch arm64 -o "$T/backend.self" "$T/backend.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1
python3 -B "$FIXTURE" emit "$T/cases"
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME OUTCOME; do
  CKIR="$T/cases/$NAME.ckir16"
  python3 -B "$REFERENCE" validate "$CKIR" >/dev/null
  for IMPL in native self; do
    python3 -B "$FIXTURE" run-filter "$T/backend.$IMPL" "$CKIR" "$T/$NAME.$IMPL.elf" 0 nonempty
    python3 -B "$FIXTURE" check-artifact "$T/$NAME.$IMPL.elf" "$CKIR"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf"
done < "$T/cases/positives.tsv"
python3 -B "$FIXTURE" mutate-less "$T/cross-high-true.native.elf" "$T/bad-less.elf"
if python3 -B "$FIXTURE" check-artifact "$T/bad-less.elf" "$T/cases/cross-high-true.ckir16" >/dev/null 2>&1; then
  echo "checked-IR-v16 backend: mutated qword Less accepted" >&2; exit 1
fi
while IFS="$TAB" read -r NAME STATUS; do
  for IMPL in native self; do
    set +e
    python3 -B "$FIXTURE" run-filter "$T/backend.$IMPL" "$T/cases/$NAME.ckir16" "$T/$NAME.$IMPL.out" "$STATUS" empty
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq 0 ] || exit "$ACTUAL"
  done
done < "$T/cases/manifest.tsv"
sh "$GATE_DIR/delta-checked-ir-v15-reference.sh"
sh "$GATE_DIR/delta-checked-ir-v14-backend.sh"
echo "checked-IR-v16 backend: PASS"
