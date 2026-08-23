#!/usr/bin/env bash
# Rebuild the Rust on-ramp CLI, then compile every sample in place, in parallel,
# via the cross-platform `omega refresh-samples` subcommand. Result: every
# samples/<domain>/<name>/build/omega-program(.exe) is current.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$root" && cargo build -p omega-cli)
exe="$root/target/debug/omega"
[ -x "$exe" ] || exe="$root/target/debug/omega.exe"
cd "$root"
exec "$exe" refresh-samples samples
