#!/usr/bin/env bash
# CI and host setup use the same source-pinned checksum-aware mbx builder.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
version=$(tr -d '[:space:]' < "$repo/tools/mbx-version")
install_dir="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/omega-mbx-$version"
bash "$repo/tools/mbx/install.sh" "$install_dir"
if [ -n "${GITHUB_PATH:-}" ]; then printf '%s\n' "$install_dir" >> "$GITHUB_PATH"; fi
