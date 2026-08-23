#!/usr/bin/env sh
# Shared-codec conformance and direct Delta O0 terminal-module emission gate.
set -e
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "omega0 terminal emission: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 || { echo "omega0 terminal emission: skipped (cargo absent)"; exit 0; }
cargo test -q -p psi-checked-trees-to-terminal --test provider_attachment_source
cargo build -q -p psi-terminal-codec --bin psi-terminal-publish
TERMINAL_PUBLISHER="$OMEGA_REPO_ROOT/target/debug/psi-terminal-publish"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "omega0 terminal emission: direct native lane skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || { echo "omega0 terminal emission: skipped (python3 absent)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
OMEGA0_WRITE_VARIANT_TERMINAL="$T/variant-shared.psi" \
  cargo test -q -p psi-checked-trees-to-terminal --test provider_attachment_source \
    source_projection_is_the_shared_o0_fixture_and_perturbations_fail_closed -- --exact
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_OMEGA0/compiler/omega0-frontend.alp" "$T/frontend" >/dev/null
python3 "$OMEGA_PATH_OMEGA0/compiler/omega0_bundle.py" pack \
  main.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" > "$T/canonical.bundle"

python3 - "$OMEGA_PATH_OMEGA0/gates/fixtures/omega0-terminal-v26.hex" "$T/frozen.psi" <<'PY'
import pathlib
import sys

source = pathlib.Path(sys.argv[1]).read_text(encoding="ascii")
pathlib.Path(sys.argv[2]).write_bytes(bytes.fromhex(source))
PY
"$TERMINAL_PUBLISHER" --success-exit 107 --expect "$T/frozen.psi" \
  "$T/emitted.psi" -- "$T/frontend" < "$T/canonical.bundle"
cmp "$T/frozen.psi" "$T/emitted.psi"

set +e
"$TERMINAL_PUBLISHER" --success-exit 107 --expect "$T/frozen.psi" \
  "$T/emitted.psi" -- python3 -c \
  'import pathlib, sys; sys.stdout.buffer.write(pathlib.Path(sys.argv[1]).read_bytes()[:-1]); sys.exit(107)' \
  "$T/frozen.psi"
TRUNCATED_STATUS=$?
set -e
[ "$TRUNCATED_STATUS" != 0 ] || {
  echo "omega0 terminal emission: truncated producer was published" >&2
  exit 1
}
cmp "$T/frozen.psi" "$T/emitted.psi"

set +e
"$TERMINAL_PUBLISHER" --success-exit 107 --expect "$T/frozen.psi" \
  "$T/emitted.psi" -- python3 -c \
  'import pathlib, sys; sys.stdout.buffer.write(pathlib.Path(sys.argv[1]).read_bytes()); sys.exit(9)' \
  "$T/frozen.psi"
FAILED_PRODUCER_STATUS=$?
set -e
[ "$FAILED_PRODUCER_STATUS" != 0 ] || {
  echo "omega0 terminal emission: failed producer was published" >&2
  exit 1
}
cmp "$T/frozen.psi" "$T/emitted.psi"

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("A\\n");self.console.exit_process(2);}' > "$T/variant.omg"
python3 "$OMEGA_PATH_OMEGA0/compiler/omega0_bundle.py" pack \
  main.omg="$T/variant.omg" > "$T/variant.bundle"
"$TERMINAL_PUBLISHER" --success-exit 77 --expect "$T/variant-shared.psi" \
  "$T/variant.psi" -- "$T/frontend" < "$T/variant.bundle"
if cmp -s "$T/frozen.psi" "$T/variant.psi"; then
  echo "omega0 terminal emission: literal/scalar perturbation did not change bytes" >&2
  exit 1
fi
cmp "$T/variant-shared.psi" "$T/variant.psi"

echo "omega0 terminal emission: fixture, direct bytes, shared decode, and verifier passed"
