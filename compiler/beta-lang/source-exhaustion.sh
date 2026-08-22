#!/usr/bin/env sh
# The bc.beta source arena is [2097152, 3145728): exactly 1 MiB. Pin the
# boundary and require an oversized input to fail before emitting a partial
# Alpha assembly program or overwriting the adjacent name tables.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"

( cd "$OMEGA_PATH_BETA_RUST" && sh build.sh "$OMEGA_PATH_BETA_LANGUAGE/bc.beta" >/dev/null ) \
  || { echo "bc source exhaustion FAIL — bc build"; exit 1; }
BC="$OMEGA_PATH_BETA_RUST/build/bc.exe"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
LIMIT=1048576

printf 'proc main() { return 0 }' > "$T/exact.beta"
PREFIX=$(wc -c < "$T/exact.beta" | tr -d ' ')
dd if=/dev/zero bs=$((LIMIT - PREFIX)) count=1 2>/dev/null | tr '\000' ' ' >> "$T/exact.beta"

set +e
"$BC" < "$T/exact.beta" > "$T/exact.asm" 2>/dev/null
exact_status=$?
set -e
if [ "$exact_status" != 0 ] || [ ! -s "$T/exact.asm" ]; then
  echo "bc source exhaustion FAIL — exact-limit input exited $exact_status or emitted no assembly"
  exit 1
fi

cp "$T/exact.beta" "$T/oversized.beta"
printf x >> "$T/oversized.beta"
set +e
"$BC" < "$T/oversized.beta" > "$T/oversized.asm" 2>/dev/null
oversized_status=$?
set -e
if [ "$oversized_status" != 253 ] || [ -s "$T/oversized.asm" ]; then
  echo "bc source exhaustion FAIL — oversized input exited $oversized_status or emitted a partial artifact"
  exit 1
fi

echo "bc source exhaustion: exact 1048576-byte input admitted; next byte rejected with empty output"
