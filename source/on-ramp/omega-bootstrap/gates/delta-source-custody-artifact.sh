#!/usr/bin/env sh
# CKIR1 artifact tranche: canonical one-unit source bundle -> checked IR ->
# deterministic Linux x86-64 ELF. This gate closes exhaustive CKIR resource and
# relation teeth plus exact independent artifact reconstruction. Lower-rooted
# source-to-CKIR-to-ELF refinement remains a separately named obligation.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "source-custody artifact: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "source-custody artifact: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "source-custody artifact: skipped ($TOOL absent)"
    exit 0
  }
done

PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
BUNDLER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_reference.py"
RESOURCE_GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_resources.py"
MUTATION_GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_mutations.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
PRODUCT_SOURCE="$OMEGA_REPO_ROOT/source/psi/source/source.omg"
for REQUIRED in "$PRODUCER" "$BACKEND" "$BUNDLER" "$REFERENCE" "$ELF_REFERENCE" \
  "$RESOURCE_GENERATOR" "$MUTATION_GENERATOR" "$FIXTURE" "$PRODUCT_SOURCE"; do
  [ -f "$REQUIRED" ] || {
    echo "source-custody artifact: required input absent: $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_SOURCE_CUSTODY_ARTIFACT_TEMP:-0}" = 1 ]; then
    echo "source-custody artifact: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"

# One timeout/capture path owns all compiler-sized and executable observations.
# Expected nonzero compiler statuses are successes of this test harness.
python3 - "$T/observe.py" <<'PY'
from pathlib import Path
import sys

Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os
import signal
import subprocess
import sys
import time

label, expected_text, timeout_text, input_name, output_name, timing_name, *command = sys.argv[1:]
expected = int(expected_text)
timeout = float(timeout_text)
started = time.monotonic()
try:
    with open(input_name, "rb") as source:
        process = subprocess.Popen(
            command,
            stdin=source,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        stdout, stderr = process.communicate(timeout=timeout)
except subprocess.TimeoutExpired:
    elapsed = time.monotonic() - started
    os.killpg(process.pid, signal.SIGKILL)
    stdout, stderr = process.communicate()
    Path(output_name).write_bytes(stdout)
    Path(output_name + ".stderr").write_bytes(stderr)
    print(f"source-custody artifact FAIL - {label} exceeded {timeout:.0f}s", file=sys.stderr)
    raise SystemExit(1)

elapsed = time.monotonic() - started
Path(output_name).write_bytes(stdout)
Path(output_name + ".stderr").write_bytes(stderr)
with open(timing_name, "a", encoding="utf-8") as timings:
    timings.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != expected:
    print(
        f"source-custody artifact FAIL - {label} status {process.returncode}, "
        f"expected {expected} ({elapsed:.2f}s)",
        file=sys.stderr,
    )
    if stderr:
        sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(1)
''', encoding="utf-8")
PY

observe() { # label expected timeout stdin stdout command...
  LABEL=$1
  EXPECTED=$2
  TIMEOUT=$3
  INPUT=$4
  OUTPUT=$5
  shift 5
  python3 "$T/observe.py" "$LABEL" "$EXPECTED" "$TIMEOUT" \
    "$INPUT" "$OUTPUT" "$T/timings.tsv" "$@"
}

assert_empty() {
  [ ! -s "$1" ] || {
    echo "source-custody artifact FAIL - $2 published bytes" >&2
    exit 1
  }
}

bundle_one() { # label source output
  observe "bundle-$1" 0 10 /dev/null "$3" \
    python3 "$BUNDLER" pack "main.omg=$2"
}

# Build the disposable Rust on-ramp once. It builds lowermachine and the two
# direct native observations; the one lowermachine executable builds both
# compiler programs for the independent self path.
observe build-delta-onramp 0 240 /dev/null "$T/cargo.stdout" \
  cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"

DELTA_ARCH=aarch64 observe build-lowermachine 0 120 /dev/null "$T/build-lowermachine.stdout" \
  "$DELTA" "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine"
DELTA_ARCH=aarch64 observe build-producer-native 0 180 /dev/null "$T/build-producer-native.stdout" \
  "$DELTA" "$PRODUCER" "$T/producer.native"
DELTA_ARCH=aarch64 observe build-backend-native 0 180 /dev/null "$T/build-backend-native.stdout" \
  "$DELTA" "$BACKEND" "$T/backend.native"

observe lower-producer 0 180 "$PRODUCER" "$T/producer.self.s" "$T/lowermachine"
observe assemble-producer 0 60 /dev/null "$T/assemble-producer.stdout" \
  clang -arch arm64 -o "$T/producer.self" "$T/producer.self.s"
observe sign-producer 0 30 /dev/null "$T/sign-producer.stdout" \
  codesign -f -s - "$T/producer.self"

observe lower-backend 0 180 "$BACKEND" "$T/backend.self.s" "$T/lowermachine"
observe assemble-backend 0 60 /dev/null "$T/assemble-backend.stdout" \
  clang -arch arm64 -o "$T/backend.self" "$T/backend.self.s"
observe sign-backend 0 30 /dev/null "$T/sign-backend.stdout" \
  codesign -f -s - "$T/backend.self"

mkdir "$T/sources"
python3 - "$FIXTURE" "$T/sources" <<'PY'
from pathlib import Path
import re
import sys

fixture = Path(sys.argv[1]).read_text(encoding="utf-8")
out = Path(sys.argv[2])

# Preserve the exact behavior while permuting root declarations and renaming
# representative semantic identifiers in the closed fixture. The sole candidate
# remains the zero-explicit-parameter scalar-result machine.
pair_at = fixture.index("data Pair")
run_at = fixture.index("machine Probe::run")
peek_comment_at = fixture.index("// Shared-receiver")
permuted = (
    fixture[pair_at:run_at]
    + fixture[:pair_at]
    + fixture[peek_comment_at:]
    + fixture[run_at:peek_comment_at]
)
# `copy` is both a field name and the spelling of the data capability. Protect
# the latter from the identifier rewrite below.
permuted = permuted.replace("[copy]", "[__copy_capability__]")
renames = {
    "Probe": "Vault", "Pair": "Duo", "run": "execute", "peek": "inspect",
    "before": "prefix", "source": "origin", "copy": "replica",
    "bytes": "cells", "length": "used", "index": "cursor",
    "after": "suffix", "retained": "kept", "first": "left", "second": "right",
    "present": "available", "fail": "rejected",
}
for old, new in renames.items():
    permuted = re.sub(rf"\b{re.escape(old)}\b", new, permuted)
permuted = permuted.replace("[__copy_capability__]", "[copy]")
(out / "renamed-reordered.omg").write_text(permuted, encoding="utf-8")

(out / "unguarded-index.omg").write_text(r'''
data Buffer { bytes: [u8; 8] in Trapping; length: u32 [0..=8]; }
machine Buffer::bad(&self, at: u32 in Trapping) -> u8 { self.bytes[at] }
''', encoding="ascii")
(out / "recursive-layout.omg").write_text(r'''
data Recursive { next: Recursive; }
''', encoding="ascii")
(out / "ambiguous-root.omg").write_text(r'''
data Left { value: u8; }
data Right { value: u8; }
machine Left::run(&self) -> u8 { 1 }
machine Right::run(&self) -> u8 { 2 }
''', encoding="ascii")

library = b"data Library { value: u8; }\n"
if len(library) > 131_072:
    raise SystemExit("internal source limit fixture")
(out / "source-limit.omg").write_bytes(library + b" " * (131_072 - len(library)))
(out / "source-over.omg").write_bytes(library + b" " * (131_073 - len(library)))
(out / "array-limit.omg").write_text(
    "data ArrayLimit { bytes: [u8; 65536] in Trapping; }\n", encoding="ascii"
)
(out / "array-over.omg").write_text(
    "data ArrayOver { bytes: [u8; 65537] in Trapping; }\n", encoding="ascii"
)
(out / "layout-limit.omg").write_text(r'''
data LayoutLimit { left: [u8; 65536] in Trapping; right: [u8; 65536] in Trapping; }
machine LayoutLimit::run(&self) -> u8 { 0 }
''', encoding="ascii")
(out / "layout-over.omg").write_text(r'''
data LayoutOver { left: [u8; 65536] in Trapping; right: [u8; 65536] in Trapping; extra: u8; }
machine LayoutOver::run(&self) -> u8 { 0 }
''', encoding="ascii")
PY

# Product-owned source checking and interpretation agrees on the fixture's
# scalar observation without reading or owning the private CKIR format.
OMEGA_SOURCE_CUSTODY_ARTIFACT=$FIXTURE observe product-source-comparator 0 240 \
  /dev/null "$T/product-source-comparator.stdout" \
  cargo test -q -p omega-native-differential-test \
  --test source_custody_artifact -- --ignored --exact \
  product_semantics_observe_source_custody_fixture

positive_case() { # label source expected-reference-observation self-producer-timeout-or-zero
  CASE=$1
  SOURCE=$2
  OBSERVATION=$3
  SELF_TIMEOUT=$4
  bundle_one "$CASE" "$SOURCE" "$T/$CASE.bundle"

  observe "$CASE-producer-native" 0 20 "$T/$CASE.bundle" "$T/$CASE.native.ckir" \
    "$T/producer.native"
  observe "$CASE-producer-repeat" 0 20 "$T/$CASE.bundle" "$T/$CASE.repeat.ckir" \
    "$T/producer.native"
  cmp "$T/$CASE.native.ckir" "$T/$CASE.repeat.ckir"
  if [ "$SELF_TIMEOUT" -ne 0 ]; then
    observe "$CASE-producer-self" 0 "$SELF_TIMEOUT" \
      "$T/$CASE.bundle" "$T/$CASE.self.ckir" "$T/producer.self"
    cmp "$T/$CASE.native.ckir" "$T/$CASE.self.ckir"
  fi
  [ -s "$T/$CASE.native.ckir" ] || {
    echo "source-custody artifact FAIL - $CASE producer emitted no CKIR" >&2
    exit 1
  }

  observe "$CASE-reference-validate" 0 15 /dev/null "$T/$CASE.validate" \
    python3 "$REFERENCE" validate "$T/$CASE.native.ckir"
  observe "$CASE-reference-run" 0 15 /dev/null "$T/$CASE.reference" \
    python3 "$REFERENCE" run "$T/$CASE.native.ckir"
  printf '%s\n' "$OBSERVATION" > "$T/$CASE.expected-reference"
  cmp "$T/$CASE.reference" "$T/$CASE.expected-reference"

  observe "$CASE-backend-native" 0 30 "$T/$CASE.native.ckir" "$T/$CASE.native.elf" \
    "$T/backend.native"
  observe "$CASE-backend-repeat" 0 30 "$T/$CASE.native.ckir" "$T/$CASE.repeat.elf" \
    "$T/backend.native"
  observe "$CASE-backend-self" 0 45 "$T/$CASE.native.ckir" "$T/$CASE.self.elf" \
    "$T/backend.self"
  cmp "$T/$CASE.native.elf" "$T/$CASE.repeat.elf"
  cmp "$T/$CASE.native.elf" "$T/$CASE.self.elf"
}

# Producer coverage is deliberately split: native+repeat is exhaustive in this
# focused gate, while the slower independent self-produced compiler covers the
# exact product closure and renamed/reordered conformance evidence. The backend
# self path is cheap enough to remain broad for every accepted CKIR below.
echo "source-custody artifact: exhaustive native producer; representative self producer"

# The real library closure and the closed structural behavior fixture are both
# canonical one-source bundles. Library CKIR is nonempty; its backend output is
# the contract's deliberately empty successful observation.
positive_case product-source "$PRODUCT_SOURCE" library 30
assert_empty "$T/product-source.native.elf" "product-source library backend"
positive_case fixture "$FIXTURE" 70 0
[ -s "$T/fixture.native.elf" ] || {
  echo "source-custody artifact FAIL - fixture backend emitted no ELF" >&2
  exit 1
}
positive_case renamed-reordered "$T/sources/renamed-reordered.omg" 70 120

# Turn the fixture's structural source place into the destination place itself.
# The mutated CKIR remains well typed and makes Copy an exact alias: semantic
# leaves must be snapshotted and padding remains outside the observation.
python3 - "$T/fixture.native.ckir" "$T/copy-self-alias.ckir" <<'PY'
from pathlib import Path
import struct
import sys

HEADER = struct.Struct("<8sHHHH14I")
OPERATION = struct.Struct("<IIIBBHIIIIII")
source = bytearray(Path(sys.argv[1]).read_bytes())
header = HEADER.unpack_from(source)
counts = header[7:]
row_sizes = (24, 20, 16, 36, 20, 32, 20, 40)
cursor = HEADER.size
for count, size in zip(counts[:7], row_sizes[:7]):
    cursor += count * size
operation_offset = cursor
operand_offset = operation_offset + counts[7] * OPERATION.size
found = 0
for operation_id in range(counts[7]):
    operation = OPERATION.unpack_from(source, operation_offset + operation_id * OPERATION.size)
    if operation[3] == 7 and operation[10] == 2:
        start = operation[8]
        destination = struct.unpack_from("<I", source, operand_offset + start * 4)[0]
        struct.pack_into("<I", source, operand_offset + (start + 1) * 4, destination)
        found += 1
if found != 1:
    raise SystemExit(f"fixture has {found} place-source Copy operations, expected one")
Path(sys.argv[2]).write_bytes(source)
PY
observe copy-self-alias-reference-validate 0 15 /dev/null "$T/copy-self-alias.validate" \
  python3 "$REFERENCE" validate "$T/copy-self-alias.ckir"
observe copy-self-alias-reference-run 0 15 /dev/null "$T/copy-self-alias.reference" \
  python3 "$REFERENCE" run "$T/copy-self-alias.ckir"
printf '71\n' > "$T/copy-self-alias.expected"
cmp "$T/copy-self-alias.reference" "$T/copy-self-alias.expected"
observe copy-self-alias-backend-native 0 30 "$T/copy-self-alias.ckir" \
  "$T/copy-self-alias.native.elf" "$T/backend.native"
observe copy-self-alias-backend-repeat 0 30 "$T/copy-self-alias.ckir" \
  "$T/copy-self-alias.repeat.elf" "$T/backend.native"
observe copy-self-alias-backend-self 0 45 "$T/copy-self-alias.ckir" \
  "$T/copy-self-alias.self.elf" "$T/backend.self"
cmp "$T/copy-self-alias.native.elf" "$T/copy-self-alias.repeat.elf"
cmp "$T/copy-self-alias.native.elf" "$T/copy-self-alias.self.elf"

# Exact and adjacent source, fixed-array, and selected-owner-layout boundaries.
positive_case source-limit "$T/sources/source-limit.omg" library 0
assert_empty "$T/source-limit.native.elf" "source-limit library backend"
positive_case array-limit "$T/sources/array-limit.omg" library 0
assert_empty "$T/array-limit.native.elf" "array-limit library backend"
positive_case layout-limit "$T/sources/layout-limit.omg" 0 0

# Reconstruct every ELF byte from CKIR rather than trusting the backend's image
# length or treating text as opaque. The fixture's byte-wide control flips each
# artifact offset once; a valid-but-different CKIR/ELF cross-pair pins relation
# custody in addition to malformed-input rejection.
for CASE in fixture renamed-reordered layout-limit; do
  observe "$CASE-elf-reconstruction" 0 15 /dev/null "$T/$CASE.elf-reference" \
    python3 "$ELF_REFERENCE" check "$T/$CASE.native.ckir" "$T/$CASE.native.elf"
done
observe copy-self-alias-elf-reconstruction 0 15 /dev/null \
  "$T/copy-self-alias.elf-reference" python3 "$ELF_REFERENCE" check \
  "$T/copy-self-alias.ckir" "$T/copy-self-alias.native.elf"
observe fixture-elf-byte-mutations 0 30 /dev/null "$T/fixture.elf-mutations" \
  python3 "$ELF_REFERENCE" mutation-sweep \
  "$T/fixture.native.ckir" "$T/fixture.native.elf"
observe mismatched-ckir-elf-relation 1 15 /dev/null "$T/mismatched-ckir-elf" \
  python3 "$ELF_REFERENCE" check \
  "$T/copy-self-alias.ckir" "$T/fixture.native.elf"

# Synthetic CKIR is appropriate for backend aggregate capacities that cannot be
# reached honestly through the much smaller source-unit/statement profile. The
# generator independently checks every canonical positive before encoding it;
# the product/reference decoder then supplies a second acceptance relation.
mkdir "$T/ckir-resources"
observe generate-ckir-resources 0 30 /dev/null "$T/generate-ckir-resources.stdout" \
  python3 "$RESOURCE_GENERATOR" "$T/ckir-resources"

TAB=$(printf '\t')
RESOURCE_COUNT=0
RESOURCE_SELF_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED_STATUS REFERENCE_VALID EXPECTED_OUTPUT SELF_REPRESENTATIVE NOTE; do
  [ "$NAME" != name ] || continue
  RESOURCE_COUNT=$((RESOURCE_COUNT + 1))
  CASE=${NAME%.ckir}
  RESOURCE_INPUT="$T/ckir-resources/$NAME"
  case "$REFERENCE_VALID" in
    true) REFERENCE_STATUS=0 ;;
    false) REFERENCE_STATUS=1 ;;
    *) echo "source-custody artifact FAIL - bad resource reference expectation: $NAME" >&2; exit 1 ;;
  esac
  observe "resource-$CASE-reference" "$REFERENCE_STATUS" 30 /dev/null \
    "$T/resource-$CASE.reference" python3 "$REFERENCE" validate "$RESOURCE_INPUT"
  observe "resource-$CASE-backend-native" "$EXPECTED_STATUS" 45 "$RESOURCE_INPUT" \
    "$T/resource-$CASE.native.output" "$T/backend.native"
  case "$EXPECTED_OUTPUT" in
    empty) assert_empty "$T/resource-$CASE.native.output" "resource $CASE native backend" ;;
    elf)
      [ -s "$T/resource-$CASE.native.output" ] || {
        echo "source-custody artifact FAIL - resource $CASE emitted no ELF" >&2
        exit 1
      }
      observe "resource-$CASE-elf-reconstruction" 0 30 /dev/null \
        "$T/resource-$CASE.elf-reference" python3 "$ELF_REFERENCE" check \
        "$RESOURCE_INPUT" "$T/resource-$CASE.native.output"
      ;;
    *) echo "source-custody artifact FAIL - bad resource output expectation: $NAME" >&2; exit 1 ;;
  esac
  if [ "$SELF_REPRESENTATIVE" = true ]; then
    RESOURCE_SELF_COUNT=$((RESOURCE_SELF_COUNT + 1))
    observe "resource-$CASE-backend-self" "$EXPECTED_STATUS" 120 "$RESOURCE_INPUT" \
      "$T/resource-$CASE.self.output" "$T/backend.self"
    case "$EXPECTED_OUTPUT" in
      empty) assert_empty "$T/resource-$CASE.self.output" "resource $CASE self backend" ;;
      elf) cmp "$T/resource-$CASE.native.output" "$T/resource-$CASE.self.output" ;;
    esac
  fi
done < "$T/ckir-resources/manifest.tsv"
[ "$RESOURCE_COUNT" -gt 0 ] && [ "$RESOURCE_SELF_COUNT" -gt 0 ] || {
  echo "source-custody artifact FAIL - empty resource coverage split" >&2
  exit 1
}

source_rejection() { # label source expected exercise-self
  CASE=$1
  SOURCE=$2
  REJECTION_STATUS=$3
  EXERCISE_SELF=$4
  bundle_one "$CASE" "$SOURCE" "$T/$CASE.bundle"
  for PRODUCER_KIND in native; do
    OUTPUT="$T/$CASE.$PRODUCER_KIND.failure"
    observe "$CASE-producer-$PRODUCER_KIND" "$REJECTION_STATUS" 30 "$T/$CASE.bundle" "$OUTPUT" \
      "$T/producer.$PRODUCER_KIND"
    assert_empty "$OUTPUT" "$CASE producer $PRODUCER_KIND"
  done
  if [ "$EXERCISE_SELF" = self ]; then
    OUTPUT="$T/$CASE.self.failure"
    observe "$CASE-producer-self" "$REJECTION_STATUS" 30 "$T/$CASE.bundle" "$OUTPUT" \
      "$T/producer.self"
    assert_empty "$OUTPUT" "$CASE producer self"
  fi
}

source_rejection unguarded-index "$T/sources/unguarded-index.omg" 251 self
source_rejection recursive-layout "$T/sources/recursive-layout.omg" 251 native-only
source_rejection ambiguous-root "$T/sources/ambiguous-root.omg" 251 native-only
source_rejection source-over "$T/sources/source-over.omg" 252 self
source_rejection array-over "$T/sources/array-over.omg" 252 native-only
source_rejection layout-over "$T/sources/layout-over.omg" 252 native-only

# Two representative malformed bundle relations: exact-EOF and declared source
# cardinality. Both producers must reject before one CKIR byte is published.
python3 - "$T/fixture.bundle" "$T/bundle-truncated" "$T/bundle-count-two" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
Path(sys.argv[2]).write_bytes(canonical[:-1])
bad_count = bytearray(canonical)
struct.pack_into("<I", bad_count, 12, 2)
Path(sys.argv[3]).write_bytes(bad_count)
PY
for CASE in bundle-truncated bundle-count-two; do
  OUTPUT="$T/$CASE.native.failure"
  observe "$CASE-producer-native" 251 30 "$T/$CASE" "$OUTPUT" \
    "$T/producer.native"
  assert_empty "$OUTPUT" "$CASE producer native"
done

# Generate schema-aware negative controls from the same all-operation fixture.
# Every mutation is rejected by the independent reference and native backend;
# the manifest-selected subset also runs through the Delta-self-built backend.
# The generator owns a fixed required-class inventory so deleting a relation
# tooth cannot silently shorten this loop.
mkdir "$T/ckir-mutations"
observe generate-ckir-mutations 0 30 /dev/null "$T/generate-ckir-mutations.stdout" \
  python3 "$MUTATION_GENERATOR" "$T/fixture.native.ckir" "$T/ckir-mutations"
EXTERNAL_CONTROL_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED_OBSERVATION REQUIRED_SHAPE; do
  [ "$NAME" != required_name ] || continue
  EXTERNAL_CONTROL_COUNT=$((EXTERNAL_CONTROL_COUNT + 1))
  awk -F '\t' -v required_name="$NAME" -v expected="$EXPECTED_OBSERVATION" '
    BEGIN { split(expected, parts, "-"); found = 0 }
    NR > 1 && $1 == required_name && $2 == parts[1] && $4 == parts[2] { found = 1 }
    END { exit found ? 0 : 1 }
  ' "$T/ckir-resources/manifest.tsv" || {
    echo "source-custody artifact FAIL - missing external mutation control: $NAME" >&2
    exit 1
  }
done < "$T/ckir-mutations/required-external-controls.tsv"
[ "$EXTERNAL_CONTROL_COUNT" -gt 0 ] || {
  echo "source-custody artifact FAIL - mutation generator named no external controls" >&2
  exit 1
}
MUTATION_COUNT=0
MUTATION_SELF_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED_STATUS MUTATION_CLASS SELF_REPRESENTATIVE; do
  [ "$NAME" != path ] || continue
  MUTATION_COUNT=$((MUTATION_COUNT + 1))
  CASE=${NAME%.ckir}
  MUTATION_INPUT="$T/ckir-mutations/$NAME"
  observe "mutation-$CASE-reference" 1 30 /dev/null \
    "$T/mutation-$CASE.reference" python3 "$REFERENCE" validate "$MUTATION_INPUT"
  observe "mutation-$CASE-backend-native" "$EXPECTED_STATUS" 30 "$MUTATION_INPUT" \
    "$T/mutation-$CASE.native.failure" "$T/backend.native"
  assert_empty "$T/mutation-$CASE.native.failure" "mutation $CASE native backend"
  if [ "$SELF_REPRESENTATIVE" = 1 ]; then
    MUTATION_SELF_COUNT=$((MUTATION_SELF_COUNT + 1))
    observe "mutation-$CASE-backend-self" "$EXPECTED_STATUS" 45 "$MUTATION_INPUT" \
      "$T/mutation-$CASE.self.failure" "$T/backend.self"
    assert_empty "$T/mutation-$CASE.self.failure" "mutation $CASE self backend"
  fi
done < "$T/ckir-mutations/manifest.tsv"
[ "$MUTATION_COUNT" -gt 0 ] && [ "$MUTATION_SELF_COUNT" -gt 0 ] || {
  echo "source-custody artifact FAIL - empty mutation coverage split" >&2
  exit 1
}

# Execute directly when this gate gains a Linux x86-64 construction path; on
# its current Darwin owner, use an available static qemu runner rather than
# silently treating structural inspection as execution.
if [ "$(uname -sm)" = "Linux x86_64" ]; then
  RUNNER=
elif command -v qemu-x86_64 >/dev/null 2>&1; then
  RUNNER=qemu-x86_64
else
  RUNNER=
fi
if [ -n "$RUNNER" ] || [ "$(uname -sm)" = "Linux x86_64" ]; then
  for CASE in fixture renamed-reordered copy-self-alias; do
    case "$CASE" in
      copy-self-alias) EXPECTED_RUNTIME=71 ;;
      *) EXPECTED_RUNTIME=70 ;;
    esac
    chmod +x "$T/$CASE.native.elf"
    if [ -n "$RUNNER" ]; then
      observe "$CASE-linux-observation" "$EXPECTED_RUNTIME" 10 /dev/null "$T/$CASE.runtime.stdout" \
        "$RUNNER" "$T/$CASE.native.elf"
    else
      observe "$CASE-linux-observation" "$EXPECTED_RUNTIME" 10 /dev/null "$T/$CASE.runtime.stdout" \
        "$T/$CASE.native.elf"
    fi
    assert_empty "$T/$CASE.runtime.stdout" "$CASE runtime stdout"
    assert_empty "$T/$CASE.runtime.stdout.stderr" "$CASE runtime stderr"
  done
else
  echo "source-custody artifact: Linux execution not available (ELF inspection completed)"
fi

python3 - "$T/timings.tsv" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    elapsed, label = line.split("\t", 1)
    rows.append((float(elapsed), label))
slowest = max(rows)
print(
    f"source-custody artifact: {len(rows)} bounded commands in "
    f"{sum(row[0] for row in rows):.2f}s; slowest {slowest[1]} {slowest[0]:.2f}s"
)
PY
echo "source-custody artifact: exhaustive native/repeat producer, representative self producer, $RESOURCE_COUNT exact/adjacent resources ($RESOURCE_SELF_COUNT self), $MUTATION_COUNT relation mutations ($MUTATION_SELF_COUNT self), and exact independent ELF reconstruction passed"
