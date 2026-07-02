#!/usr/bin/env bash
# Rebuild the CLI (a SEPARATE workspace -- `cargo build --workspace` never
# relinks it, so it silently runs stale compiler code), then compile every
# sample in place, in parallel, via the cross-platform `omega refresh-samples`
# subcommand. Result: every samples/<name>/build/omega-program(.exe) is current.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$root/apps/omega-cli" && cargo build)
exe="$root/target/debug/omega"
[ -x "$exe" ] || exe="$root/target/debug/omega.exe"
cd "$root"
exec "$exe" refresh-samples samples
