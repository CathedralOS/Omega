#!/usr/bin/env bash
# Rebuild the Rust development CLI, then compile every sample in place, in parallel,
# via the cross-platform `omega refresh-samples` subcommand. Result: every
# samples/<domain>/<name>/build/omega-program(.exe) is current.
set -euo pipefail

version=$(mbx --version 2>/dev/null) || {
  echo "error: mbx 1.7.0 or newer is required; direct Cargo fallback is forbidden" >&2
  exit 1
}
number=${version#mbx }
major=${number%%.*}
remainder=${number#*.}
minor=${remainder%%.*}
case "$major" in
  ''|*[!0-9]*)
    echo "error: could not parse mbx version: $version" >&2
    exit 1
    ;;
esac
case "$minor" in
  ''|*[!0-9]*)
    echo "error: could not parse mbx version: $version" >&2
    exit 1
    ;;
esac
if [ "$major" -lt 1 ] || { [ "$major" -eq 1 ] && [ "$minor" -lt 7 ]; }; then
  echo "error: mbx 1.7.0 or newer is required; found $version" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$root" && mbx build -p omega)
exe="$root/target/debug/omega"
[ -x "$exe" ] || exe="$root/target/debug/omega.exe"
cd "$root"
exec "$exe" refresh-samples samples
