#!/usr/bin/env sh
# OMGCOMP2 structural custody gate for the exact Linux-x64 Console exit carrier.
# Provider semantics and authority are deliberately outside this gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "OMGCOMP2 custody: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

for TOOL in python3 cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGCOMP2 custody: $TOOL required" >&2
    exit 2
  }
done

PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_compilation_v2.py"
CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-compilation-check.alp"
FIXTURE="$GATE_DIR/fixtures/omgcomp2-console-exit-provider"
REFERENCE="$GATE_DIR/omgcomp2_console_exit_fixture.py"
[ -f "$PACKER" ] && [ -f "$CHECKER" ] && [ -f "$REFERENCE" ] || {
  echo "OMGCOMP2 custody: implementation input absent" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -m py_compile "$PACKER" "$REFERENCE"
python3 "$REFERENCE" build "$T/cases" > "$T/reference.sha256"
python3 "$REFERENCE" check "$T/cases"
cmp "$T/cases/reference.sha256" "$T/reference.sha256"
python3 "$PACKER" pack "$FIXTURE/manifest.json" "$T/cases/source.bundle" > "$T/packed.omgc"
cmp "$T/cases/reference.omgc" "$T/packed.omgc"
python3 "$PACKER" verify "$T/packed.omgc"
python3 "$PACKER" inspect "$T/packed.omgc" > "$T/inspection.json"
cmp "$T/cases/inspection.json" "$T/inspection.json"

# Preserve the complete V1 pack/verify/inspect behavior rather than relying on
# the one inherited V1 structural case below.
"$GATE_DIR/omega-bootstrap-compilation-test.sh"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *)
    echo "OMGCOMP2 custody: PASS reference/packer/V1; native/self skipped (requires Darwin arm64)"
    exit 0
    ;;
esac
for TOOL in cargo clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGCOMP2 custody: PASS reference/packer/V1; native/self skipped ($TOOL absent)"
    exit 0
  }
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$CHECKER" "$T/checker.native" >/dev/null
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine" >/dev/null

python3 - "$T/lowermachine" "$CHECKER" "$T/checker.self.s" <<'PY'
import subprocess
import sys

lowermachine, source, output = sys.argv[1:]
with open(source, "rb") as stdin, open(output, "wb") as stdout:
    try:
        result = subprocess.run(
            [lowermachine],
            stdin=stdin,
            stdout=stdout,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit("OMGCOMP2 custody: lowermachine exceeded 60 seconds")
if result.returncode != 0:
    raise SystemExit(
        "OMGCOMP2 custody: lowermachine failed: "
        + result.stderr.decode("utf-8", errors="replace")[:240]
    )
PY
clang -arch arm64 -o "$T/checker.self" "$T/checker.self.s"
codesign -f -s - "$T/checker.self" >/dev/null 2>&1

python3 - "$T/checker.native" "$T/checker.self" "$T/cases/cases.tsv" <<'PY'
from pathlib import Path
import subprocess
import sys

native, self_built, manifest = sys.argv[1:]
rows = [line.split("\t") for line in Path(manifest).read_text().splitlines()]
for implementation in (native, self_built):
    for name, expected_text, input_name in rows:
        expected = int(expected_text)
        with open(input_name, "rb") as stdin:
            result = subprocess.run(
                [implementation], stdin=stdin, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, timeout=10, check=False,
            )
        if result.returncode != expected or result.stdout != b"":
            raise SystemExit(
                f"OMGCOMP2 custody: {Path(implementation).name}/{name}: "
                f"expected status {expected} and empty output, got "
                f"{result.returncode} and {result.stdout[:120]!r}"
            )
print(f"OMGCOMP2 custody: native/self PASS {len(rows)} exact 0/251/252 cases")
PY

echo "OMGCOMP2 custody: PASS"
