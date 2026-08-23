#!/usr/bin/env sh
# Delta-written O0 artifact edge: canonical terminal-Psi -> exact x86-64 ELF.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "delta O0 artifact: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 || { echo "delta O0 artifact: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "delta O0 artifact: skipped (python3 absent)"; exit 0; }

case "$(uname -sm)" in
  "Darwin arm64") DELTA_NATIVE=aarch64 ;;
  "Linux x86_64") DELTA_NATIVE=x86_64 ;;
  *) echo "delta O0 artifact: native Delta execution skipped on $(uname -sm)"; exit 0 ;;
esac

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
if [ "$DELTA_NATIVE" = aarch64 ]; then
  DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
    "$OMEGA_PATH_OMEGA0/compiler/omega0-terminal-to-elf.alp" "$T/backend" >/dev/null
else
  "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
    "$OMEGA_PATH_OMEGA0/compiler/omega0-terminal-to-elf.alp" "$T/backend" >/dev/null
fi

python3 - "$OMEGA_PATH_OMEGA0/gates/fixtures/omega0-terminal-v25.hex" "$T/terminal.psi" <<'PY'
import pathlib
import sys
pathlib.Path(sys.argv[2]).write_bytes(bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii")))
PY

"$T/backend" < "$T/terminal.psi" > "$T/delta.elf"
OMEGA0_WRITE_X64_IMAGE="$T/rust.elf" cargo test -q -p omega-native-differential-test \
  --test omega0_runnable \
  canonical_omega0_agrees_from_terminal_meaning_through_runnable_linux_image -- --exact
cmp "$T/delta.elf" "$T/rust.elf"

OMEGA0_WRITE_VARIANT_TERMINAL="$T/variant.psi" cargo test -q \
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
  [ "$STATUS" -ne 0 ] || { echo "delta O0 artifact: accepted $bad input" >&2; exit 1; }
  [ ! -s "$T/$bad.elf" ] || { echo "delta O0 artifact: emitted bytes for $bad input" >&2; exit 1; }
done

[ "$(wc -c < "$T/delta.elf" | tr -d ' ')" = 8192 ] || {
  echo "delta O0 artifact: unexpected ELF size" >&2
  exit 1
}

echo "delta O0 artifact: exact Rust match, operand variant, and fail-closed controls passed"
