#!/usr/bin/env sh
# Exact O1 backend differential: native Delta execution versus the Rust-free
# omega2gamma.beta -> canonical Gamma interpreter route.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "delta O1 artifact meaning: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 || { echo "delta O1 artifact meaning: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "delta O1 artifact meaning: python3 required" >&2; exit 2; }
command -v perl >/dev/null 2>&1 || { echo "delta O1 artifact meaning: perl required for hard timeouts" >&2; exit 2; }

case "$(uname -sm)" in
  "Darwin arm64") DELTA_NATIVE=aarch64 ;;
  "Linux x86_64") DELTA_NATIVE=x86_64 ;;
  *) echo "delta O1 artifact meaning: native Delta execution skipped on $(uname -sm)"; exit 0 ;;
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

stamp_beta_compiler "$T/bc.exe" >/dev/null \
  || { echo "delta O1 artifact meaning FAIL — Beta compiler artifact"; exit 1; }
BC="$T/bc.exe"
ASM="$OMEGA_PATH_BETA_ASSEMBLER/$BETA_SEED"
build_beta() {
  "$BC" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA0/meaning/omega2gamma.beta" "$T/elaborate.exe" \
  || { echo "delta O1 artifact meaning FAIL — omega2gamma build"; exit 1; }
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" \
  || { echo "delta O1 artifact meaning FAIL — Gamma interpreter build"; exit 1; }

# Compiler-sized elaboration is itself bounded evidence. Reuse this one result
# for every input so the gate measures backend meaning, not repeated translation.
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" \
  < "$OMEGA_PATH_OMEGA0/compiler/omega0-terminal-to-elf.alp" > "$T/backend.gamma"
[ -s "$T/backend.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/backend.gamma" \
  || { echo "delta O1 artifact meaning FAIL — backend elaboration unsupported"; exit 1; }
gamma_bytes=$(wc -c < "$T/backend.gamma" | tr -d ' ')
[ "$gamma_bytes" -le 1048576 ] \
  || { echo "delta O1 artifact meaning FAIL — backend Gamma expanded to $gamma_bytes bytes"; exit 1; }

# Product-owned exporters provide the canonical accepted variant and both O1
# exhaustion controls. The frozen O0 terminal bytes remain an independent fixture.
OMEGA1_WRITE_TERMINAL_REFERENCES="$T/terminal-refs" cargo test -q \
  -p psi-checked-trees-to-terminal --test provider_attachment_source \
  straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes -- --exact
OMEGA0_WRITE_X64_IMAGE="$T/product.elf" cargo test -q \
  -p omega-native-differential-test --test omega0_runnable \
  canonical_omega0_agrees_from_terminal_meaning_through_runnable_linux_image -- --exact
OMEGA0_WRITE_VARIANT_TERMINAL="$T/variant.psi" cargo test -q \
  -p psi-checked-trees-to-terminal --test provider_attachment_source \
  source_projection_is_the_shared_o0_fixture_and_perturbations_fail_closed -- --exact

python3 - "$OMEGA_PATH_OMEGA0/gates/fixtures/omega0-terminal-v25.hex" \
  "$T/canonical.psi" "$T/tampered.psi" <<'PY'
import pathlib
import sys

canonical = bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii"))
pathlib.Path(sys.argv[2]).write_bytes(canonical)
tampered = bytearray(canonical)
tampered[0] ^= 1
pathlib.Path(sys.argv[3]).write_bytes(tampered)
PY

gamma_program() {
  python3 - "$T/backend.gamma" "$1" "$2" <<'PY'
import pathlib
import sys

program = pathlib.Path(sys.argv[1]).read_text(encoding="ascii")
source = pathlib.Path(sys.argv[2]).read_bytes()
stdin = "Nil"
for byte in reversed(source):
    stdin = f"(Cons {byte} {stdin})"
if program.count("STDIN") != 1:
    raise SystemExit("backend meaning: expected one STDIN placeholder")
pathlib.Path(sys.argv[3]).write_text(program.replace("STDIN", stdin), encoding="ascii")
PY
}

PASS=0
run_case() { # label terminal expected-status
  label=$1
  terminal=$2
  expected=$3
  stem=$T/case-$PASS

  set +e
  "$T/backend" < "$terminal" > "$stem.native"
  native_status=$?
  set -e

  gamma_program "$terminal" "$stem.gamma"
  set +e
  perl -e 'alarm 20; exec @ARGV' "$T/interp.exe" < "$stem.gamma" > "$stem.observation"
  interp_status=$?
  set -e
  [ "$interp_status" -eq 0 ] || {
    echo "delta O1 artifact meaning FAIL — $label Gamma interpreter exited $interp_status" >&2
    exit 1
  }
  gamma_status=$(python3 "$OMEGA_PATH_OMEGA0/meaning/decode-gamma-output.py" \
    "$stem.observation" "$stem.lower-rung")

  [ "$native_status" -eq "$expected" ] || {
    echo "delta O1 artifact meaning FAIL — $label native status $native_status, expected $expected" >&2
    exit 1
  }
  [ "$gamma_status" -eq "$expected" ] || {
    echo "delta O1 artifact meaning FAIL — $label lower-rung status $gamma_status, expected $expected" >&2
    exit 1
  }
  cmp "$stem.native" "$stem.lower-rung" || {
    echo "delta O1 artifact meaning FAIL — $label artifact bytes differ" >&2
    exit 1
  }
  LAST_NATIVE=$stem.native
  PASS=$((PASS + 1))
}

run_case "canonical O0" "$T/canonical.psi" 0
cmp "$LAST_NATIVE" "$T/product.elf"
[ "$(wc -c < "$LAST_NATIVE" | tr -d ' ')" -eq 8192 ]

run_case "operand variant" "$T/variant.psi" 0
run_case "malformed magic" "$T/tampered.psi" 251
[ ! -s "$LAST_NATIVE" ]
run_case "write-count exhaustion" "$T/terminal-refs/reject-writes-17.terminal" 252
[ ! -s "$LAST_NATIVE" ]
run_case "aggregate-text exhaustion" "$T/terminal-refs/reject-bytes-1200.terminal" 252
[ ! -s "$LAST_NATIVE" ]

echo "delta O1 artifact meaning: $PASS exact native/lower-rung observations passed ($gamma_bytes-byte elaboration)"
