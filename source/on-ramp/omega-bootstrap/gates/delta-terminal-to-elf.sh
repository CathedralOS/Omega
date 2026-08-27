#!/usr/bin/env sh
# Delta-written bounded artifact edge: O1/scalar terminal-Psi -> x86-64 ELF.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "delta bounded artifact: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 || { echo "delta bounded artifact: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "delta bounded artifact: skipped (python3 absent)"; exit 0; }

case "$(uname -sm)" in
  "Darwin arm64") DELTA_NATIVE=aarch64 ;;
  "Linux x86_64")
    echo "delta bounded artifact: native producer skipped (the current x64 Delta on-ramp emits Windows PE); Rust-free Gamma gate owns Linux execution"
    exit 0
    ;;
  *) echo "delta bounded artifact: native Delta execution skipped on $(uname -sm)"; exit 0 ;;
esac

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega-bootstrap-terminal-to-elf.alp" "$T/backend" >/dev/null

# The profile-neutral scalar lane is distinguished from O1 by its empty top-
# level structural tables. Exercise general IDs and both call directions, then
# require every semantic/malformed and capacity boundary to fail before output.
SCALAR_FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-scalar-call-v28.hex"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/scalar-call-terminal-cases.py" \
  "$SCALAR_FIXTURE" "$T/scalar-cases"

for TERMINAL in "$T/scalar-cases/accepted"/*.psi; do
  CASE=$(basename "$TERMINAL" .psi)
  "$T/backend" < "$TERMINAL" > "$T/scalar-$CASE.elf"
done
cmp "$T/scalar-canonical.elf" "$T/scalar-arbitrary-machine-ids.elf"

for TERMINAL in "$T/scalar-cases/accepted-boundary"/*.psi; do
  CASE=$(basename "$TERMINAL" .psi)
  "$T/backend" < "$TERMINAL" > "$T/scalar-boundary-$CASE.elf"
done

python3 - "$T" <<'PY'
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
expected = bytes.fromhex(
    "e80b00000089c7b8e70000000f050f0b"
    "554889e54881ec00020000"
    "b8490000008985fcffffff"
    "8bbdfcffffffe80e0000008985f8ffffff"
    "8b85f8ffffffc9c3"
    "554889e54881ec00020000"
    "89bdfcffffff8b85fcffffffc9c3"
)
assert len(expected) == 88
for name in (
    "scalar-canonical.elf",
    "scalar-arbitrary-machine-ids.elf",
    "scalar-machine-order-permutation.elf",
):
    path = root / name
    image = path.read_bytes()
    assert len(image) == 8192, path
    assert image[:4] == b"\x7fELF", path
    assert struct.unpack_from("<H", image, 18)[0] == 62, path
    assert struct.unpack_from("<Q", image, 24)[0] == 0x401000, path
    assert struct.unpack_from("<Q", image, 96)[0] == 4096 + 88, path
    assert image[4096 + 88:] == bytes(4096 - 88), path

canonical = (root / "scalar-canonical.elf").read_bytes()[4096:4096 + 88]
assert canonical == expected

for path in sorted(root.glob("scalar-boundary-*.elf")):
    image = path.read_bytes()
    assert len(image) == 8192, path
    assert image[:4] == b"\x7fELF", path
    text_size = struct.unpack_from("<Q", image, 96)[0] - 4096
    assert 0 < text_size <= 4096, (path, text_size)

# The permuted terminal puts callee machine 2 first. The entry shim reaches the
# later caller and that caller's rel32 reaches backward to the earlier callee.
permuted = (root / "scalar-machine-order-permutation.elf").read_bytes()[4096:4096 + 88]
entry_disp = struct.unpack_from("<i", permuted, 1)[0]
assert 5 + entry_disp == 41
calls = [index for index, byte in enumerate(permuted) if byte == 0xE8]
assert calls == [0, 69]
call_disp = struct.unpack_from("<i", permuted, 70)[0]
assert 74 + call_disp == 16
PY

for TERMINAL in "$T/scalar-cases/reject-251"/*.psi; do
  CASE=$(basename "$TERMINAL" .psi)
  set +e
  "$T/backend" < "$TERMINAL" > "$T/scalar-reject-$CASE.elf"
  scalar_rc=$?
  set -e
  [ "$scalar_rc" -eq 251 ] && [ ! -s "$T/scalar-reject-$CASE.elf" ] || {
    echo "delta bounded artifact: scalar $CASE did not reject 251/empty" >&2
    exit 1
  }
done

for TERMINAL in "$T/scalar-cases/reject-252"/*.psi; do
  CASE=$(basename "$TERMINAL" .psi)
  set +e
  "$T/backend" < "$TERMINAL" > "$T/scalar-exhaust-$CASE.elf"
  scalar_rc=$?
  set -e
  [ "$scalar_rc" -eq 252 ] && [ ! -s "$T/scalar-exhaust-$CASE.elf" ] || {
    echo "delta bounded artifact: scalar $CASE did not exhaust 252/empty" >&2
    exit 1
  }
done

if [ "$(uname -sm)" = "Linux x86_64" ]; then
  for IMAGE in "$T"/scalar-{canonical,arbitrary-machine-ids,machine-order-permutation}.elf; do
    chmod +x "$IMAGE"
    set +e
    "$IMAGE" > "$IMAGE.stdout" 2> "$IMAGE.stderr"
    scalar_rc=$?
    set -e
    [ "$scalar_rc" -eq 73 ] && [ ! -s "$IMAGE.stdout" ] && [ ! -s "$IMAGE.stderr" ] || {
      echo "delta bounded artifact: scalar image observation drifted" >&2
      exit 1
    }
  done
fi

# Independently project canonical terminal modules, run their terminal meaning
# and product lowering, then require the Delta backend to reproduce every x64
# image byte-for-byte. The two exporters deliberately travel through different
# product tests; their terminal bytes must also agree.
OMEGA1_WRITE_TERMINAL_REFERENCES="$T/terminal-refs" cargo test -q \
  -p psi-checked-trees-to-terminal --test provider_attachment_source \
  straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes -- --exact
OMEGA1_WRITE_REFERENCE_DIR="$T/product-refs" cargo test -q \
  -p omega-native-differential-test --test omega_bootstrap_runnable \
  straight_line_console_o1_agrees_for_zero_one_two_and_sixteen_writes -- --exact

for WRITES in 0 1 2 16; do
  CASE="writes-$WRITES"
  cmp "$T/terminal-refs/$CASE.terminal" "$T/product-refs/$CASE.terminal"
  "$T/backend" < "$T/terminal-refs/$CASE.terminal" > "$T/$CASE.delta.elf"
  cmp "$T/$CASE.delta.elf" "$T/product-refs/$CASE.x86_64.elf"
done

python3 - "$T" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
for writes in (0, 1, 2, 16):
    image = (root / f"writes-{writes}.delta.elf").read_bytes()
    text_size = 53 * writes + 7 * writes + (15 if writes else 14)
    assert len(image) == 8192
    assert int.from_bytes(image[96:104], "little") == 4096 + text_size
    assert image[4096 + text_size:] == bytes(4096 - text_size)
    (root / f"writes-{writes}.stdout").write_bytes(
        b"".join(f"line-{index:02}\n".encode() for index in range(writes))
    )
PY

if [ "$(uname -sm)" = "Linux x86_64" ]; then
  for WRITES in 0 1 2 16; do
    CASE="writes-$WRITES"
    chmod +x "$T/$CASE.delta.elf"
    set +e
    "$T/$CASE.delta.elf" > "$T/$CASE.actual.stdout"
    STATUS=$?
    set -e
    [ "$STATUS" -eq "$WRITES" ] || {
      echo "delta bounded artifact: $CASE exited $STATUS, expected $WRITES" >&2
      exit 1
    }
    cmp "$T/$CASE.actual.stdout" "$T/$CASE.stdout"
  done
fi

# Canonical terminal Psi beyond either declared O1 ceiling must exhaust before
# publishing even one artifact byte.
for CASE in reject-writes-17 reject-bytes-1200; do
  set +e
  "$T/backend" < "$T/terminal-refs/$CASE.terminal" > "$T/$CASE.elf"
  STATUS=$?
  set -e
  [ "$STATUS" -eq 252 ] || {
    echo "delta bounded artifact: $CASE exited $STATUS, expected checked exhaustion 252" >&2
    exit 1
  }
  [ ! -s "$T/$CASE.elf" ] || {
    echo "delta bounded artifact: emitted bytes for $CASE" >&2
    exit 1
  }
done

# Keep the frozen O0 fixture tied to its original exact product image as an
# explicit monotonic-compatibility check.
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-terminal-v28.hex" "$T/terminal.psi" <<'PY'
import pathlib
import sys
pathlib.Path(sys.argv[2]).write_bytes(bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii")))
PY

"$T/backend" < "$T/terminal.psi" > "$T/delta.elf"
OMEGA_BOOTSTRAP_WRITE_X64_IMAGE="$T/rust.elf" cargo test -q -p omega-native-differential-test \
  --test omega_bootstrap_runnable \
  canonical_o0_agrees_from_terminal_meaning_through_runnable_linux_image -- --exact
cmp "$T/delta.elf" "$T/rust.elf"

OMEGA_BOOTSTRAP_WRITE_VARIANT_TERMINAL="$T/variant.psi" cargo test -q \
  -p psi-checked-trees-to-terminal --test provider_attachment_source \
  source_projection_is_the_shared_o0_fixture_and_perturbations_fail_closed -- --exact
"$T/backend" < "$T/variant.psi" > "$T/variant.elf"
python3 - "$T/variant.elf" <<'PY'
import pathlib
import sys
image = pathlib.Path(sys.argv[1]).read_bytes()
assert len(image) == 8192
assert int.from_bytes(image[96:104], "little") == 4166
assert image[4148:4151] == b"A\n\n"
assert image[4151:4156] == b"\xbf\x02\x00\x00\x00"
PY

if [ "$(uname -sm)" = "Linux x86_64" ]; then
  chmod +x "$T/variant.elf"
  set +e
  "$T/variant.elf" > "$T/variant.stdout"
  STATUS=$?
  set -e
  [ "$STATUS" -eq 2 ] && cmp "$T/variant.stdout" - <<'EOF'
A

EOF
fi

python3 - "$T/terminal.psi" "$T/truncated.psi" "$T/tampered.psi" \
  "$T/trailing.psi" "$T/root-schema-tampered.psi" <<'PY'
import pathlib
import sys
source = pathlib.Path(sys.argv[1]).read_bytes()
pathlib.Path(sys.argv[2]).write_bytes(source[:-1])
tampered = bytearray(source)
tampered[0] ^= 1
pathlib.Path(sys.argv[3]).write_bytes(tampered)
pathlib.Path(sys.argv[4]).write_bytes(source + b"\0")

# Exit boundary identity (147 bytes), then attachment, scalar parameters,
# structural parameters, result, and requirements. O0/O1 must publish an empty
# program-local-root-introduction schema table at the following u32.
prefix = b"named-callable(path(Console::exit_process)"
assert source.count(prefix) == 1
schema_count = source.index(prefix) + 147 + 18
assert source[schema_count:schema_count + 4] == b"\0\0\0\0"
schema_tampered = bytearray(source)
schema_tampered[schema_count] = 1
pathlib.Path(sys.argv[5]).write_bytes(schema_tampered)
PY

for bad in truncated tampered trailing root-schema-tampered; do
  set +e
  "$T/backend" < "$T/$bad.psi" > "$T/$bad.elf"
  STATUS=$?
  set -e
  [ "$STATUS" -eq 251 ] || {
    echo "delta bounded artifact: $bad input exited $STATUS, expected malformed status 251" >&2
    exit 1
  }
  [ ! -s "$T/$bad.elf" ] || { echo "delta bounded artifact: emitted bytes for $bad input" >&2; exit 1; }
done

[ "$(wc -c < "$T/delta.elf" | tr -d ' ')" = 8192 ] || {
  echo "delta bounded artifact: unexpected ELF size" >&2
  exit 1
}

echo "delta bounded artifact: scalar calls/relocations/teeth plus O1 0/1/2/16 writes, exact product images, ceilings, and fail-closed controls passed"
