#!/usr/bin/env sh
# Focused persisted-Beta/Gamma meaning probe for CKIR4 -> Linux x86-64 ELF.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v4 backend meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v4 backend meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v4 backend meaning: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v4-to-elf.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v4-fixture.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v4_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v4_reference.py"
RUNNER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-ckir4-meaning-runner.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$BACKEND" "$FIXTURE" "$REFERENCE" "$ELF_REFERENCE" "$RUNNER" "$DECODER"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v4 backend meaning: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "checked-IR-v4 backend meaning FAIL - Beta compiler artifact" >&2
  exit 1
}
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "checked-IR-v4 backend meaning FAIL - omega2gamma build" >&2; exit 1;
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "checked-IR-v4 backend meaning FAIL - Gamma interpreter build" >&2; exit 1;
}

# Translate the backend exactly once and reuse the result for 0/251/252.
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$BACKEND" \
  "$T/backend.gamma" "$T/timings.tsv" "checked-IR-v4 backend meaning" 25 700000

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
python3 -B "$FIXTURE" emit "$T/cases"
cp "$T/cases/canonical.ckir4" "$T/canonical.ckir4"
cp "$T/cases/schema-major-3.ckir4" "$T/semantic-251.ckir4"
cp "$T/cases/constructor-five-valid.ckir4" "$T/resource-252.ckir4"

[ "$(python3 -B "$REFERENCE" run "$T/canonical.ckir4")" = 70 ] || {
  echo "checked-IR-v4 backend meaning FAIL - canonical CKIR4 result is not 70" >&2
  exit 1
}
native_case() { # label expected-status
  set +e
  "$T/backend.native" < "$T/$1.ckir4" > "$T/$1.expected"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$2" ] || {
    echo "checked-IR-v4 backend meaning FAIL - $1 native status $STATUS, expected $2" >&2
    exit 1
  }
  if [ "$2" -ne 0 ] && [ -s "$T/$1.expected" ]; then
    echo "checked-IR-v4 backend meaning FAIL - $1 native rejection published bytes" >&2
    exit 1
  fi
}
native_case canonical 0
native_case semantic-251 251
native_case resource-252 252
python3 -B "$ELF_REFERENCE" check "$T/canonical.ckir4" "$T/canonical.expected" >/dev/null

launch_gamma() { # label timeout
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/backend.gamma" \
    "$T/$1.ckir4" "$T/$1.observation" "$T/timings.tsv" \
    "checked-IR-v4 backend meaning $1" "$2"
}
check_gamma() { # label expected-status
  STATUS=$(python3 -B "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq "$2" ] || {
    echo "checked-IR-v4 backend meaning FAIL - $1 status $STATUS, expected $2" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$T/$1.expected" >/dev/null || {
    echo "checked-IR-v4 backend meaning FAIL - $1 publication differs" >&2
    exit 1
  }
}
launch_gamma canonical 300 & CANONICAL_PID=$!
launch_gamma semantic-251 90 & SEMANTIC_PID=$!
launch_gamma resource-252 240 & RESOURCE_PID=$!
set +e
wait "$CANONICAL_PID"; CANONICAL_WAIT=$?
wait "$SEMANTIC_PID"; SEMANTIC_WAIT=$?
wait "$RESOURCE_PID"; RESOURCE_WAIT=$?
set -e
[ "$CANONICAL_WAIT" -eq 0 ] && [ "$SEMANTIC_WAIT" -eq 0 ] && \
  [ "$RESOURCE_WAIT" -eq 0 ] || {
  echo "checked-IR-v4 backend meaning FAIL - Gamma child status canonical=$CANONICAL_WAIT semantic=$SEMANTIC_WAIT resource=$RESOURCE_WAIT" >&2
  exit 1
}
check_gamma canonical 0
check_gamma semantic-251 251
check_gamma resource-252 252
python3 -B "$ELF_REFERENCE" check "$T/canonical.ckir4" "$T/canonical.stdout" >/dev/null

python3 - "$T/timings.tsv" "$T/canonical.expected" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds, size, label = line.split("\t", 2)
    rows.append(f"{label}={float(seconds):.2f}s/{size}B")
print(
    "checked-IR-v4 backend meaning: exact nested ConstructRecord -> Call -> Copy ELF/result 70; "
    "schema rejection=251 and constructor exhaustion=252 publish nothing; canonical Gamma agrees; "
    + " ".join(rows)
    + f" ELF={Path(sys.argv[2]).stat().st_size}B"
)
PY
