#!/usr/bin/env sh
# Frozen O1 dependency closure: lowermachine-built frontend -> vocabulary-28
# terminal Psi -> lowermachine-built direct backend -> exact Linux x86-64 ELF.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta O1 self-host composite: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Delta O1 self-host composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "Delta O1 self-host composite: skipped ($TOOL absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

# The disposable Rust on-ramp produces only the initial lowermachine executable.
# Both compiler programs under test are then compiled by that Delta-written
# compiler and assembled as native Darwin tools for this gate.
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine" >/dev/null

compile_through_lowermachine() {
  SOURCE=$1
  OUTPUT=$2
  "$T/lowermachine" < "$SOURCE" > "$OUTPUT.s"
  clang -arch arm64 -o "$OUTPUT" "$OUTPUT.s"
  codesign -f -s - "$OUTPUT" >/dev/null 2>&1
}

compile_through_lowermachine \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-frontend.alp" "$T/frontend"
compile_through_lowermachine \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-terminal-to-elf.alp" "$T/backend"

# These exporters independently travel through the product source-to-terminal
# and terminal-to-image paths. The gate uses them only as differential fixtures.
OMEGA1_WRITE_TERMINAL_REFERENCES="$T/terminal-refs" cargo test -q \
  -p psi-checked-trees-to-terminal --test provider_attachment_source \
  straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes -- --exact
OMEGA1_WRITE_REFERENCE_DIR="$T/product-refs" cargo test -q \
  -p omega-native-differential-test --test omega_bootstrap_runnable \
  straight_line_console_o1_agrees_for_zero_one_two_and_sixteen_writes -- --exact

bundle_one() {
  python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" \
    pack main.omg="$1" > "$2"
}

write_numbered_source() {
  COUNT=$1
  OUTPUT=$2
  {
    printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
    INDEX=0
    while [ "$INDEX" -lt "$COUNT" ]; do
      printf 'self.console.write_line("line-%02d");' "$INDEX"
      INDEX=$((INDEX + 1))
    done
    printf 'self.console.exit_process(%d);}' "$COUNT"
  } > "$OUTPUT"
}

expected_digest() {
  case "$1" in
    0) printf 0 ;;
    1) printf 64 ;;
    2) printf 129 ;;
    16) printf 86 ;;
    *) return 1 ;;
  esac
}

for WRITES in 0 1 2 16; do
  CASE="writes-$WRITES"
  write_numbered_source "$WRITES" "$T/$CASE.omg"
  bundle_one "$T/$CASE.omg" "$T/$CASE.bundle"

  set +e
  "$T/frontend" < "$T/$CASE.bundle" > "$T/$CASE.terminal"
  STATUS=$?
  set -e
  EXPECTED=$(expected_digest "$WRITES")
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "Delta O1 self-host composite: $CASE frontend status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  cmp "$T/$CASE.terminal" "$T/terminal-refs/$CASE.terminal"
  cmp "$T/$CASE.terminal" "$T/product-refs/$CASE.terminal"

  "$T/backend" < "$T/$CASE.terminal" > "$T/$CASE.elf"
  cmp "$T/$CASE.elf" "$T/product-refs/$CASE.x86_64.elf"
done

# Pre-profile bundle transport: auxiliary source units are retained and
# validated independently, but do not change the one O1 program's terminal or
# native artifact bytes.
: > "$T/empty.omg"
printf '// auxiliary transport unit' > "$T/comment.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a/empty.omg="$T/empty.omg" m/program.omg="$T/writes-1.omg" \
  z/comment.omg="$T/comment.omg" > "$T/auxiliary.bundle"
set +e
"$T/frontend" < "$T/auxiliary.bundle" > "$T/auxiliary.terminal"
AUXILIARY_STATUS=$?
set -e
[ "$AUXILIARY_STATUS" -eq 64 ] || {
  echo "Delta O1 self-host composite: auxiliary bundle status $AUXILIARY_STATUS, expected 64" >&2
  exit 1
}
cmp "$T/auxiliary.terminal" "$T/writes-1.terminal"
"$T/backend" < "$T/auxiliary.terminal" > "$T/auxiliary.elf"
cmp "$T/auxiliary.elf" "$T/writes-1.elf"
cmp "$T/auxiliary.elf" "$T/product-refs/writes-1.x86_64.elf"

run_frontend_bundle_rejection() {
  LABEL=$1
  BUNDLE=$2
  EXPECTED=$3
  set +e
  "$T/frontend" < "$BUNDLE" > "$T/$LABEL.terminal"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "Delta O1 self-host composite: $LABEL status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$LABEL.terminal" ] || {
    echo "Delta O1 self-host composite: $LABEL published terminal bytes" >&2
    exit 1
  }
}
run_frontend_rejection() {
  LABEL=$1
  SOURCE=$2
  EXPECTED=$3
  bundle_one "$SOURCE" "$T/$LABEL.bundle"
  run_frontend_bundle_rejection "$LABEL" "$T/$LABEL.bundle" "$EXPECTED"
}

# Semantic rejection must stop composition before a terminal module exists.
printf '%s' 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(0);self.console.write_line("x");}' \
  > "$T/bad-order.omg"
run_frontend_rejection bad-order "$T/bad-order.omg" 251

# Each frozen frontend resource ceiling exhausts without partial publication.
dd if=/dev/zero of="$T/source-exhaust.omg" bs=2049 count=1 2>/dev/null
run_frontend_rejection source-exhaust "$T/source-exhaust.omg" 252

{
  printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
  INDEX=0
  while [ "$INDEX" -lt 17 ]; do
    printf 'self.console.write_line("");'
    INDEX=$((INDEX + 1))
  done
  printf 'self.console.exit_process(0);}'
} > "$T/write-exhaust.omg"
run_frontend_rejection write-exhaust "$T/write-exhaust.omg" 252

{
  printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
  INDEX=0
  while [ "$INDEX" -lt 15 ]; do
    printf 'self.console.write_line("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");'
    INDEX=$((INDEX + 1))
  done
  printf 'self.console.write_line("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");self.console.exit_process(0);}'
} > "$T/text-exhaust.omg"
run_frontend_rejection text-exhaust "$T/text-exhaust.omg" 252

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a.omg="$T/writes-0.omg" b.omg="$T/writes-1.omg" > "$T/two-program.bundle"
run_frontend_bundle_rejection two-program "$T/two-program.bundle" 251

printf '\377' > "$T/invalid-auxiliary.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a/program.omg="$T/writes-1.omg" z/invalid.omg="$T/invalid-auxiliary.omg" \
  > "$T/invalid-auxiliary.bundle"
run_frontend_bundle_rejection invalid-auxiliary "$T/invalid-auxiliary.bundle" 251

python3 - "$T/descriptor-exhaust.bundle" "$T/content-exhaust.bundle" <<'PY'
import pathlib
import struct
import sys

magic = b"OMG0BNDL"
pathlib.Path(sys.argv[1]).write_bytes(struct.pack("<8sII", magic, 1, 17))
pathlib.Path(sys.argv[2]).write_bytes(
    struct.pack("<8sII", magic, 1, 2)
    + struct.pack("<II", 1, 1) + b"a "
    + struct.pack("<II", 1, 2048)
)
PY
run_frontend_bundle_rejection descriptor-exhaust "$T/descriptor-exhaust.bundle" 252
run_frontend_bundle_rejection content-exhaust "$T/content-exhaust.bundle" 252

run_backend_rejection() {
  LABEL=$1
  INPUT=$2
  EXPECTED=$3
  set +e
  "$T/backend" < "$INPUT" > "$T/$LABEL.elf"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "Delta O1 self-host composite: $LABEL backend status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$LABEL.elf" ] || {
    echo "Delta O1 self-host composite: $LABEL published image bytes" >&2
    exit 1
  }
}

run_backend_rejection reject-writes-17 \
  "$T/terminal-refs/reject-writes-17.terminal" 252
run_backend_rejection reject-bytes-1200 \
  "$T/terminal-refs/reject-bytes-1200.terminal" 252

python3 - "$T/writes-1.terminal" "$T/truncated.terminal" "$T/tampered.terminal" "$T/trailing.terminal" <<'PY'
import pathlib
import sys

source = pathlib.Path(sys.argv[1]).read_bytes()
pathlib.Path(sys.argv[2]).write_bytes(source[:-1])
tampered = bytearray(source)
tampered[0] ^= 1
pathlib.Path(sys.argv[3]).write_bytes(tampered)
pathlib.Path(sys.argv[4]).write_bytes(source + b"\0")
PY

run_backend_rejection truncated "$T/truncated.terminal" 251
run_backend_rejection tampered "$T/tampered.terminal" 251
run_backend_rejection trailing "$T/trailing.terminal" 251

echo "Delta O1 self-host composite: lowermachine-built frontend/backend, exact 0/1/2/16 and auxiliary-bundle terminal/ELF bytes, and fail-closed controls passed"
