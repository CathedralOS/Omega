#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Beta word prefix: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Beta word prefix: requires macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

PREFIX_TMP=$(mktemp -d)
trap 'rm -rf -- "$PREFIX_TMP"' EXIT HUP INT TERM
materialize_beta_compiler "$PREFIX_TMP/compiler" >/dev/null
python3 -B "$GATE_DIR/word-prefix.py" "$PREFIX_TMP/compiler"
