#!/usr/bin/env sh
# Delta-written O1 artifact edge: bounded straight-line terminal-Psi -> exact x86-64 ELF.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "delta O1 artifact: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 || { echo "delta O1 artifact: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "delta O1 artifact: skipped (python3 absent)"; exit 0; }

case "$(uname -sm)" in
  "Darwin arm64") DELTA_NATIVE=aarch64 ;;
  "Linux x86_64") DELTA_NATIVE=x86_64 ;;
  *) echo "delta O1 artifact: native Delta execution skipped on $(uname -sm)"; exit 0 ;;
esac

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
if [ "$DELTA_NATIVE" = aarch64 ]; then
  DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
    "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-terminal-to-elf.alp" "$T/backend" >/dev/null
else
  "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
    "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-terminal-to-elf.alp" "$T/backend" >/dev/null
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
      echo "delta O1 artifact: $CASE exited $STATUS, expected $WRITES" >&2
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
    echo "delta O1 artifact: $CASE exited $STATUS, expected checked exhaustion 252" >&2
    exit 1
  }
  [ ! -s "$T/$CASE.elf" ] || {
    echo "delta O1 artifact: emitted bytes for $CASE" >&2
    exit 1
  }
done

# Keep the frozen O0 fixture tied to its original exact product image as an
# explicit monotonic-compatibility check.
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-terminal-v26.hex" "$T/terminal.psi" <<'PY'
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

python3 - "$T/terminal.psi" "$T/truncated.psi" "$T/tampered.psi" "$T/trailing.psi" <<'PY'
import pathlib
import sys
source = pathlib.Path(sys.argv[1]).read_bytes()
pathlib.Path(sys.argv[2]).write_bytes(source[:-1])
tampered = bytearray(source)
tampered[0] ^= 1
pathlib.Path(sys.argv[3]).write_bytes(tampered)
pathlib.Path(sys.argv[4]).write_bytes(source + b"\0")
PY

for bad in truncated tampered trailing; do
  set +e
  "$T/backend" < "$T/$bad.psi" > "$T/$bad.elf"
  STATUS=$?
  set -e
  [ "$STATUS" -eq 251 ] || {
    echo "delta O1 artifact: $bad input exited $STATUS, expected malformed status 251" >&2
    exit 1
  }
  [ ! -s "$T/$bad.elf" ] || { echo "delta O1 artifact: emitted bytes for $bad input" >&2; exit 1; }
done

[ "$(wc -c < "$T/delta.elf" | tr -d ' ')" = 8192 ] || {
  echo "delta O1 artifact: unexpected ELF size" >&2
  exit 1
}

echo "delta O1 artifact: 0/1/2/16 writes, exact product images, ceilings, O0 identity, and fail-closed controls passed"
