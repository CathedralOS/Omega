#!/usr/bin/env sh
# Reconstruct the accepted checker twice below Beta and compare the committed
# tape byte-for-byte. This is a deterministic checker-lineage gate.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

sh "$SCRIPT_DIR/construct-artifact.sh" "$TMP/check-1.tape"
sh "$SCRIPT_DIR/construct-artifact.sh" "$TMP/check-2.tape"
cmp "$TMP/check-1.tape" "$TMP/check-2.tape"
cmp "$TMP/check-1.tape" "$SCRIPT_DIR/artifacts/check.tape"

OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$SCRIPT_DIR/artifact_env.sh"
stamp_proof_checker "$TMP/check" >/dev/null
[ "$(printf '%s' '(-> P P) (lam P (hyp 0))' | "$TMP/check")" = accept ]
[ "$(printf '%s' '(-> P Q) (lam P (hyp 0))' | "$TMP/check")" = reject ]

echo "Alpha-rooted checker reconstruction OK ($(wc -c < "$SCRIPT_DIR/artifacts/check.tape" | tr -d ' ')-byte artifact)"
