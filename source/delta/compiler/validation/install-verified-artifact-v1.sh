#!/usr/bin/env sh
# Install one fully reconstructed Delta compiler result without rebuilding it.
set -eu

if [ "$#" -ne 27 ]; then
  echo "usage: $0 DESTINATION ARTIFACT_RECEIPT ASSEMBLY_RECEIPT REALIZATION_OBSERVATION ARTIFACT REALIZATION_STDOUT REALIZATION_STDERR CLANG LINKER SDK_SETTINGS LIBSYSTEM_STUB COMPILER_RUNTIME ASSEMBLER_TAPE TRANSLATOR_TAPE INTERPRETER_TAPE TEMPLATE GAMMA ELAB_OBS ELAB_ERR EXEC0_OBS EXEC0_RAW EXEC0_ASM EXEC0_ERR EXEC1_OBS EXEC1_RAW EXEC1_ASM EXEC1_ERR" >&2
  exit 2
fi

DESTINATION=$1; shift
ARTIFACT_RECEIPT=$1; shift
ASSEMBLY_RECEIPT=$1; shift
REALIZATION_OBSERVATION=$1; shift
ARTIFACT=$1; shift
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
EXEC0_RAW=$1; shift
EXEC0_ASM=$1; shift
EXEC0_ERR=$1; shift
EXEC1_OBS=$1; shift
EXEC1_RAW=$1; shift
EXEC1_ASM=$1; shift
EXEC1_ERR=$1; shift

require_absolute() {
  case "$1" in
    /*) ;;
    *) echo "Delta artifact install V1: path must be absolute: $1" >&2; exit 2 ;;
  esac
}

for path in "$DESTINATION" "$ARTIFACT_RECEIPT" "$ASSEMBLY_RECEIPT" \
  "$REALIZATION_OBSERVATION" "$ARTIFACT" "$REALIZATION_STDOUT" \
  "$REALIZATION_STDERR" "$CLANG" "$LINKER" "$SDK_SETTINGS" \
  "$LIBSYSTEM_STUB" "$COMPILER_RUNTIME" "$ASSEMBLER_TAPE" \
  "$TRANSLATOR_TAPE" "$INTERPRETER_TAPE" "$TEMPLATE" "$GAMMA" \
  "$ELAB_OBS" "$ELAB_ERR" "$EXEC0_OBS" "$EXEC0_RAW" "$EXEC0_ASM" \
  "$EXEC0_ERR" "$EXEC1_OBS" "$EXEC1_RAW" "$EXEC1_ASM" "$EXEC1_ERR"; do
  require_absolute "$path"
done

case "$DESTINATION" in
  */darwin-arm64-v1) ;;
  *) echo "Delta artifact install V1: destination must end in darwin-arm64-v1" >&2; exit 2 ;;
esac

[ ! -e "$DESTINATION" ] || {
  echo "Delta artifact install V1: destination already exists: $DESTINATION" >&2
  exit 2
}

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
VERIFY_INSTALLED=$GATE_DIR/verify-installed-artifact-v1.sh
DESTINATION_PARENT=$(dirname -- "$DESTINATION")
mkdir -p "$DESTINATION_PARENT"
STAGING_ROOT=$(mktemp -d "$DESTINATION_PARENT/.delta-artifact-install-v1.XXXXXX")
STAGING=$STAGING_ROOT/darwin-arm64-v1
mkdir "$STAGING"
cleanup() {
  [ -n "${STAGING_ROOT:-}" ] && [ -d "$STAGING_ROOT" ] && rm -rf "$STAGING_ROOT"
}
trap cleanup EXIT HUP INT TERM

cp "$ARTIFACT" "$STAGING/delta-compiler"
cp "$ASSEMBLY_RECEIPT" "$STAGING/assembly-publication-receipt.json"
cp "$REALIZATION_OBSERVATION" "$STAGING/realization-observation.json"
cp "$ARTIFACT_RECEIPT" "$STAGING/artifact-custody-receipt.json"
cp "$EXEC0_RAW" "$STAGING/execution.raw"
chmod 0755 "$STAGING/delta-compiler"
chmod 0644 "$STAGING/assembly-publication-receipt.json" \
  "$STAGING/realization-observation.json" \
  "$STAGING/artifact-custody-receipt.json" "$STAGING/execution.raw"

python3 -B - "$STAGING" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
names = (
    "artifact-custody-receipt.json",
    "assembly-publication-receipt.json",
    "delta-compiler",
    "execution.raw",
    "realization-observation.json",
)
ceilings = {
    "artifact-custody-receipt.json": 65_536,
    "assembly-publication-receipt.json": 65_536,
    "delta-compiler": 64 * 1024 * 1024,
    "execution.raw": 256 * 1024 * 1024,
    "realization-observation.json": 65_536,
}
files = []
for name in names:
    path = root / name
    extent = path.stat().st_size
    if extent > ceilings[name]:
        print(f"Delta artifact install V1: {name} byte ceiling", file=sys.stderr)
        raise SystemExit(252)
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while True:
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
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
(root / "installation.json").write_text(
    json.dumps(manifest, indent=2, sort_keys=True) + "\n"
)
PY
chmod 0644 "$STAGING/installation.json"

"$VERIFY_INSTALLED" "$STAGING" "$REALIZATION_STDOUT" "$REALIZATION_STDERR" \
  "$CLANG" "$LINKER" "$SDK_SETTINGS" "$LIBSYSTEM_STUB" "$COMPILER_RUNTIME" \
  "$ASSEMBLER_TAPE" "$TRANSLATOR_TAPE" "$INTERPRETER_TAPE" "$TEMPLATE" \
  "$GAMMA" "$ELAB_OBS" "$ELAB_ERR" "$EXEC0_OBS" "$EXEC0_ASM" \
  "$EXEC0_ERR" "$EXEC1_OBS" "$EXEC1_RAW" "$EXEC1_ASM" "$EXEC1_ERR"

mv -n "$STAGING" "$DESTINATION"
[ ! -d "$STAGING" ] || {
  echo "Delta artifact install V1: destination appeared during installation" >&2
  exit 2
}
rmdir "$STAGING_ROOT"
STAGING_ROOT=
trap - EXIT HUP INT TERM
echo "Delta artifact install V1 PASS — installed $DESTINATION"
