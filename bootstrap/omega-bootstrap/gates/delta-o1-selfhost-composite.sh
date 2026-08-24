#!/usr/bin/env sh
# Bounded bridge dependency closure: lowermachine-built frontend -> vocabulary-28
# terminal Psi -> lowermachine-built direct backend -> exact Linux x86-64 ELF
# for frozen O1 and the profile-neutral scalar call/return conformance slice.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta bounded self-host composite: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Delta bounded self-host composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "Delta bounded self-host composite: skipped ($TOOL absent)"
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
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-frontend.alp" "$T/native-frontend" >/dev/null
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-terminal-to-elf.alp" "$T/native-backend" >/dev/null

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

# The general scalar source path must survive both Delta compiler producers and
# reproduce the exact product-owned terminal fixture. The Delta backends then
# agree byte-for-byte on the conservative runnable image; this image has the
# bridge's direct-entry layout, so it is compared with the product reference by
# behavior in its owning gate rather than conflated with the product's owned
# appended entry shim.
printf '%s' 'machine caller() -> i32 { return passthrough(73); } machine passthrough(value: i32) -> i32 { return value; }' > "$T/scalar.omg"
bundle_one "$T/scalar.omg" "$T/scalar.bundle"
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-scalar-call-v28.hex" "$T/scalar-reference.terminal" <<'PY'
import pathlib
import sys
pathlib.Path(sys.argv[2]).write_bytes(
    bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii"))
)
PY
for FRONTEND in "$T/native-frontend" "$T/frontend"; do
  "$FRONTEND" < "$T/scalar.bundle" > "$FRONTEND.scalar.terminal"
  cmp "$FRONTEND.scalar.terminal" "$T/scalar-reference.terminal"
done
"$T/native-backend" < "$T/scalar-reference.terminal" > "$T/native-scalar.elf"
"$T/backend" < "$T/scalar-reference.terminal" > "$T/self-scalar.elf"
cmp "$T/native-scalar.elf" "$T/self-scalar.elf"

run_scalar_composite_case() {
  scalar_label=$1
  scalar_source=$2
  printf '%s' "$scalar_source" > "$T/$scalar_label.omg"
  bundle_one "$T/$scalar_label.omg" "$T/$scalar_label.bundle"
  "$T/native-frontend" < "$T/$scalar_label.bundle" > "$T/$scalar_label.native.terminal"
  "$T/frontend" < "$T/$scalar_label.bundle" > "$T/$scalar_label.self.terminal"
  cmp "$T/$scalar_label.native.terminal" "$T/$scalar_label.self.terminal"
  "$T/native-backend" < "$T/$scalar_label.native.terminal" > "$T/$scalar_label.native.elf"
  "$T/backend" < "$T/$scalar_label.self.terminal" > "$T/$scalar_label.self.elf"
  cmp "$T/$scalar_label.native.elf" "$T/$scalar_label.self.elf"
}
run_scalar_composite_case scalar-nested \
  'machine root()->i32{return outer(inner(7));} machine outer(x:i32)->i32{return x;} machine inner(y:i32)->i32{return y;}'
run_scalar_composite_case scalar-four-arguments \
  'machine root()->i32{return fourth(1,2,3,4);} machine fourth(a:i32,b:i32,c:i32,d:i32)->i32{return d;}'
run_scalar_composite_case scalar-minimum \
  'machine root()->i32{return -2147483648;}'
run_scalar_composite_case scalar-maximum \
  'machine root()->i32{return 2147483647;}'

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
    echo "Delta bounded self-host composite: $CASE frontend status $STATUS, expected $EXPECTED" >&2
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
printf '// auxiliary transport /* remains line text' > "$T/comment.omg"
printf '/* outer auxiliary // remains block text /* nested */ tail */' > "$T/block-comment.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a/empty.omg="$T/empty.omg" b/block.omg="$T/block-comment.omg" \
  m/program.omg="$T/writes-1.omg" \
  z/comment.omg="$T/comment.omg" > "$T/auxiliary.bundle"
set +e
"$T/frontend" < "$T/auxiliary.bundle" > "$T/auxiliary.terminal"
AUXILIARY_STATUS=$?
set -e
[ "$AUXILIARY_STATUS" -eq 64 ] || {
  echo "Delta bounded self-host composite: auxiliary bundle status $AUXILIARY_STATUS, expected 64" >&2
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
    echo "Delta bounded self-host composite: $LABEL status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$LABEL.terminal" ] || {
    echo "Delta bounded self-host composite: $LABEL published terminal bytes" >&2
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

printf '%s' 'machine root()->i32{return missing(1);}' > "$T/scalar-unknown.omg"
run_frontend_rejection scalar-unknown "$T/scalar-unknown.omg" 251

make_scalar_chain() {
  scalar_count=$1
  scalar_output=$2
  : > "$scalar_output"
  scalar_index=0
  while [ "$scalar_index" -lt "$scalar_count" ]; do
    scalar_next=$((scalar_index + 1))
    if [ "$scalar_next" -lt "$scalar_count" ]; then
      printf 'machine m%d()->i32{return m%d();}' "$scalar_index" "$scalar_next" >> "$scalar_output"
    else
      printf 'machine m%d()->i32{return 0;}' "$scalar_index" >> "$scalar_output"
    fi
    scalar_index=$scalar_next
  done
}
make_scalar_chain 17 "$T/scalar-machines-17.omg"
run_frontend_rejection scalar-machines-17 "$T/scalar-machines-17.omg" 252

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

printf '/*' > "$T/comment-open.omg"
printf '*/' > "$T/comment-close.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a/open.omg="$T/comment-open.omg" b/close.omg="$T/comment-close.omg" \
  z/program.omg="$T/writes-1.omg" > "$T/cross-unit-comment.bundle"
run_frontend_bundle_rejection cross-unit-comment "$T/cross-unit-comment.bundle" 251

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
    echo "Delta bounded self-host composite: $LABEL backend status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$LABEL.elf" ] || {
    echo "Delta bounded self-host composite: $LABEL published image bytes" >&2
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

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/scalar-call-terminal-cases.py" \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-scalar-call-v28.hex" \
  "$T/scalar-cases"
run_backend_rejection scalar-unknown-callee \
  "$T/scalar-cases/reject-251/unknown-callee.psi" 251
run_backend_rejection scalar-machine-count-17 \
  "$T/scalar-cases/reject-252/machine-count-17.psi" 252

echo "Delta bounded self-host composite: lowermachine-built frontend/backend, exact scalar Call plus O1 0/1/2/16 terminal/ELF bytes, and fail-closed controls passed"
