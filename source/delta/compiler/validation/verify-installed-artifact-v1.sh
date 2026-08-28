#!/usr/bin/env sh
# Verify one installed Delta compiler artifact from all reconstruction evidence.
# The installation manifest is an inventory only; authority remains with the
# assembly-publication and artifact-custody verifiers invoked below.
set -eu

if [ "$#" -ne 22 ]; then
  echo "usage: $0 INSTALLATION REALIZATION_STDOUT REALIZATION_STDERR CLANG LINKER SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME ASSEMBLER_TAPE TRANSLATOR_TAPE INTERPRETER_TAPE TEMPLATE GAMMA ELAB_OBS ELAB_ERR EXEC0_OBS EXEC0_ASM EXEC0_ERR EXEC1_OBS EXEC1_RAW EXEC1_ASM EXEC1_ERR" >&2
  exit 2
fi

INSTALLATION=$1; shift
REALIZATION_STDOUT=$1; shift
REALIZATION_STDERR=$1; shift
CLANG=$1; shift
LINKER=$1; shift
SDK_SETTINGS=$1; shift
LIBSYSTEM_STUB=$1; shift
COMPILER_RUNTIME=$1; shift
ASSEMBLER_TAPE=$1; shift
TRANSLATOR_TAPE=$1; shift
INTERPRETER_TAPE=$1; shift
TEMPLATE=$1; shift
GAMMA=$1; shift
ELAB_OBS=$1; shift
ELAB_ERR=$1; shift
EXEC0_OBS=$1; shift
EXEC0_ASM=$1; shift
EXEC0_ERR=$1; shift
EXEC1_OBS=$1; shift
EXEC1_RAW=$1; shift
EXEC1_ASM=$1; shift
EXEC1_ERR=$1; shift

require_absolute() {
  case "$1" in
    /*) ;;
    *) echo "Delta installed artifact V1: path must be absolute: $1" >&2; exit 2 ;;
  esac
}

for path in "$INSTALLATION" "$REALIZATION_STDOUT" "$REALIZATION_STDERR" \
  "$CLANG" "$LINKER" "$SDK_SETTINGS" "$LIBSYSTEM_STUB" \
  "$COMPILER_RUNTIME" "$ASSEMBLER_TAPE" "$TRANSLATOR_TAPE" \
  "$INTERPRETER_TAPE" "$TEMPLATE" "$GAMMA" "$ELAB_OBS" "$ELAB_ERR" \
  "$EXEC0_OBS" "$EXEC0_ASM" "$EXEC0_ERR" "$EXEC1_OBS" "$EXEC1_RAW" \
  "$EXEC1_ASM" "$EXEC1_ERR"; do
  require_absolute "$path"
done

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta installed artifact V1: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"

case "$INSTALLATION" in
  */darwin-arm64-v1) ;;
  *) echo "Delta installed artifact V1: installation must end in darwin-arm64-v1" >&2; exit 2 ;;
esac

ARTIFACT=$INSTALLATION/delta-compiler
ASSEMBLY_RECEIPT=$INSTALLATION/assembly-publication-receipt.json
REALIZATION_OBSERVATION=$INSTALLATION/realization-observation.json
ARTIFACT_RECEIPT=$INSTALLATION/artifact-custody-receipt.json
EXEC0_RAW=$INSTALLATION/execution.raw
INSTALLATION_MANIFEST=$INSTALLATION/installation.json

python3 -B - "$INSTALLATION" <<'PY'
import hashlib
import json
import os
import stat
import sys
from pathlib import Path

root = Path(sys.argv[1])
def fail(message, status=251):
    print(f"Delta installed artifact V1: {message}", file=sys.stderr)
    raise SystemExit(status)

names = (
    "artifact-custody-receipt.json",
    "assembly-publication-receipt.json",
    "delta-compiler",
    "execution.raw",
    "installation.json",
    "realization-observation.json",
)
try:
    root_mode = root.lstat().st_mode
    if not stat.S_ISDIR(root_mode) or stat.S_ISLNK(root_mode):
        fail("installation directory")
    if tuple(sorted(os.listdir(root))) != names:
        fail("installation inventory")
except OSError as error:
    fail(f"installation inventory: {error}")

files = []
ceilings = {
    "artifact-custody-receipt.json": 65_536,
    "assembly-publication-receipt.json": 65_536,
    "delta-compiler": 64 * 1024 * 1024,
    "execution.raw": 256 * 1024 * 1024,
    "realization-observation.json": 65_536,
}
for name in names:
    if name == "installation.json":
        continue
    path = root / name
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        fail("non-regular installed file")
    if before.st_size > ceilings[name]:
        fail(f"{name} byte ceiling", 252)
    digest = hashlib.sha256()
    extent = 0
    with path.open("rb") as stream:
        opened_before = os.fstat(stream.fileno())
        while True:
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            extent += len(chunk)
            digest.update(chunk)
        opened_after = os.fstat(stream.fileno())
    after = path.lstat()
    identities = (
        (before.st_dev, before.st_ino, before.st_size),
        (opened_before.st_dev, opened_before.st_ino, opened_before.st_size),
        (opened_after.st_dev, opened_after.st_ino, opened_after.st_size),
        (after.st_dev, after.st_ino, after.st_size),
    )
    if len(set(identities)) != 1 or extent != before.st_size:
        fail(f"{name} changed while reading")
    files.append({
        "byte_length": extent,
        "name": name,
        "sha256": digest.hexdigest(),
    })

manifest = {
    "files": files,
    "schema": "omega.delta-compiler-installation-manifest.v1",
    "target": "darwin-arm64-v1",
}
expected = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
manifest_path = root / "installation.json"
manifest_stat = manifest_path.lstat()
if (
    not stat.S_ISREG(manifest_stat.st_mode)
    or stat.S_ISLNK(manifest_stat.st_mode)
    or manifest_stat.st_size > 65_536
    or manifest_path.read_bytes() != expected
):
    fail("installation manifest")
if not os.access(root / "delta-compiler", os.X_OK):
    fail("compiler is not executable")
PY

VERIFY=$OMEGA_PATH_DELTA_VALIDATION/lower_rooted_artifact_custody_v1.py
MANIFEST=$OMEGA_PATH_DELTA_VALIDATION/source-closures/canonical-compiler-v1.json
LOCATIONS=$OMEGA_PATH_DELTA_VALIDATION/source-closures/canonical-compiler-v1.locations.json

python3 -B "$VERIFY" verify \
  "$ARTIFACT_RECEIPT" "$ASSEMBLY_RECEIPT" "$REALIZATION_OBSERVATION" \
  "$EXEC0_ASM" "$ARTIFACT" "$REALIZATION_STDOUT" "$REALIZATION_STDERR" \
  "$CLANG" "$LINKER" "$SDK_SETTINGS" "$LIBSYSTEM_STUB" "$COMPILER_RUNTIME" \
  "$MANIFEST" "$LOCATIONS" "$ASSEMBLER_TAPE" "$TRANSLATOR_TAPE" \
  "$INTERPRETER_TAPE" "$TEMPLATE" "$GAMMA" "$ELAB_OBS" "$ELAB_ERR" \
  "$EXEC0_OBS" "$EXEC0_RAW" "$EXEC0_ASM" "$EXEC0_ERR" \
  "$EXEC1_OBS" "$EXEC1_RAW" "$EXEC1_ASM" "$EXEC1_ERR" \
  "delta=$OMEGA_PATH_DELTA"

echo "Delta installed artifact V1 PASS — exact retained artifact/evidence inventory and full reconstruction"
