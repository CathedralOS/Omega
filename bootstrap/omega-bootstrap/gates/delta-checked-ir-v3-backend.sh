#!/usr/bin/env sh
# Focused CKIR3 -> deterministic Linux x86-64 ELF backend gate. It begins at
# independently generated CKIR3 bytes and does not claim source lowering.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v3 backend: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v3 backend: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign rg; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v3 backend: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v3-backend-fixture.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$BACKEND" "$FIXTURE" "$IR_REFERENCE" "$ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v3 backend: required input absent: $REQUIRED" >&2
    exit 1
  }
done

PROCEDURES=$(rg -c '^machine ' "$BACKEND")
[ "$PROCEDURES" -lt 128 ] || {
  echo "checked-IR-v3 backend: $PROCEDURES procedures exceeds Delta envelope" >&2
  exit 1
}

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_CKIR3_BACKEND_TEMP:-0}" = 1 ]; then
    echo "checked-IR-v3 backend: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT

STARTED=$(python3 -c 'import time; print(time.time())')
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
"$T/lowermachine" < "$BACKEND" > "$T/backend.self.s"
clang -arch arm64 -o "$T/backend.self" "$T/backend.self.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1
BUILT=$(python3 -c 'import time; print(time.time())')

run_expect() { # executable input expected-status output label
  set +e
  "$1" < "$2" > "$4"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "checked-IR-v3 backend FAIL - $5 returned $ACTUAL, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "checked-IR-v3 backend FAIL - $5 published bytes on rejection" >&2
    exit 1
  fi
}

python3 -B "$FIXTURE" emit "$T/canonical.ckir3"
python3 -B "$FIXTURE" emit-no-pool "$T/no-pool.ckir3"
python3 -B "$FIXTURE" emit-image-boundary "$T/image-boundary.ckir3"
python3 -B "$FIXTURE" emit-image-resource "$T/image-resource.ckir3"
python3 -B "$FIXTURE" mutations "$T/cases"
[ "$(python3 -B "$FIXTURE" check "$T/canonical.ckir3")" = 70 ]

for BACKEND_EXE in "$T/backend.native" "$T/backend.self"; do
  run_expect "$BACKEND_EXE" "$T/canonical.ckir3" 0 "$T/canonical.$(basename "$BACKEND_EXE").elf" "canonical"
  run_expect "$BACKEND_EXE" "$T/no-pool.ckir3" 0 "$T/no-pool.$(basename "$BACKEND_EXE").elf" "no-pool"
  run_expect "$BACKEND_EXE" "$T/image-boundary.ckir3" 0 "$T/image-boundary.$(basename "$BACKEND_EXE").elf" "image boundary"
  run_expect "$BACKEND_EXE" "$T/image-resource.ckir3" 252 "$T/image-resource.$(basename "$BACKEND_EXE").out" "image resource"
done
cmp "$T/canonical.backend.native.elf" "$T/canonical.backend.self.elf"
cmp "$T/no-pool.backend.native.elf" "$T/no-pool.backend.self.elf"
cmp "$T/image-boundary.backend.native.elf" "$T/image-boundary.backend.self.elf"
python3 -B "$FIXTURE" check-elf "$T/canonical.ckir3" "$T/canonical.backend.native.elf" >/dev/null
python3 -B "$FIXTURE" check-no-pool-elf "$T/no-pool.ckir3" "$T/no-pool.backend.native.elf" >/dev/null
python3 -B "$FIXTURE" check-image-boundary-elf "$T/image-boundary.ckir3" "$T/image-boundary.backend.native.elf" >/dev/null
[ "$(python3 -B "$IR_REFERENCE" run "$T/canonical.ckir3")" = 70 ]
[ "$(python3 -B "$IR_REFERENCE" run "$T/no-pool.ckir3")" = 70 ]
python3 -B "$ELF_REFERENCE" mutation-sweep "$T/canonical.ckir3" "$T/canonical.backend.native.elf" >/dev/null
python3 -B "$ELF_REFERENCE" check "$T/no-pool.ckir3" "$T/no-pool.backend.native.elf" >/dev/null
python3 -B "$ELF_REFERENCE" check "$T/image-boundary.ckir3" "$T/image-boundary.backend.native.elf" >/dev/null

# Repeated publication must be byte-identical.
run_expect "$T/backend.native" "$T/canonical.ckir3" 0 "$T/canonical.repeat.elf" "canonical repeat"
cmp "$T/canonical.backend.native.elf" "$T/canonical.repeat.elf"

MALFORMED_CASES='schema-major
constant-count-extent constant-child-count-extent
constant-dense-id constant-empty-span constant-span-start constant-span-count
constant-reserved constant-scalar-range constant-structural-scalar
constant-scalar-type constant-structural-type constant-order-inversion
constant-order-duplicate constant-forward-child constant-unreachable
copy-opcode copy-flags copy-destination-operand copy-operand-count
copy-result-kind copy-result-id copy-result-type copy-root-id copy-root-type
copy-immediate-one less-equal-immediate less-equal-result-kind
less-equal-result-id less-equal-result-type'
for CASE in $MALFORMED_CASES; do
  run_expect "$T/backend.native" "$T/cases/$CASE.ckir3" 251 "$T/$CASE.native.out" "$CASE native"
  run_expect "$T/backend.self" "$T/cases/$CASE.ckir3" 251 "$T/$CASE.self.out" "$CASE self"
  if python3 -B "$IR_REFERENCE" validate "$T/cases/$CASE.ckir3" \
      > "$T/$CASE.reference.out" 2> "$T/$CASE.reference.stderr"; then
    echo "checked-IR-v3 backend FAIL - independent reference accepted $CASE" >&2
    exit 1
  fi
done
for CASE in constant-node-resource constant-child-resource encoded-byte-resource; do
  run_expect "$T/backend.native" "$T/cases/$CASE.ckir3" 252 "$T/$CASE.native.out" "$CASE native"
  run_expect "$T/backend.self" "$T/cases/$CASE.ckir3" 252 "$T/$CASE.self.out" "$CASE self"
  if python3 -B "$IR_REFERENCE" validate "$T/cases/$CASE.ckir3" \
      > "$T/$CASE.reference.out" 2> "$T/$CASE.reference.stderr"; then
    echo "checked-IR-v3 backend FAIL - independent reference accepted $CASE" >&2
    exit 1
  fi
done

FINISHED=$(python3 -c 'import time; print(time.time())')
python3 - "$STARTED" "$BUILT" "$FINISHED" "$PROCEDURES" \
  "$(wc -c < "$T/backend.self.s" | tr -d ' ')" <<'PY'
import sys

started, built, finished = map(float, sys.argv[1:4])
print(
    "checked-IR-v3 backend: "
    f"{sys.argv[4]} procedures; self asm {sys.argv[5]} bytes; "
    f"build {built-started:.2f}s, controls {finished-built:.2f}s, "
    f"total {finished-started:.2f}s; nested constant/<=, conditional R segment, "
    "canonical DAG/image boundary, 30 isolated 251 mutations, separate 252 "
    "resources, native/self/reference controls passed"
)
PY
