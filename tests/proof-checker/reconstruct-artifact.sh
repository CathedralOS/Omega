#!/usr/bin/env sh
# Reconstruct the accepted checker once through the canonical Gamma compiler and compare the committed
# tape byte-for-byte. A second identical run would measure reproducibility but
# would add no source/artifact or derivation-validity premise.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
sh "$OMEGA_REPO_ROOT/tools/bootstrap/proof-checker/construct-artifact.sh" "$TMP/check.tape"
cmp "$TMP/check.tape" "$OMEGA_PATH_ALPHA_CHECKER/artifacts/proof_checker_bytecode.tape"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proof-checker/artifact_env.sh"
stamp_proof_checker "$TMP/check" >/dev/null
[ "$(printf '%s' '(-> P P) (lam P (hyp 0))' | "$TMP/check")" = accept ]
[ "$(printf '%s' '(-> P Q) (lam P (hyp 0))' | "$TMP/check")" = reject ]

# D23 makes realistic maximum-subject acceptance part of checker construction,
# not an optional tape-only capacity diagnostic. The gate stamps the artifact
# just compared above and pins the exact/adjacent V2 frame and arena boundaries.
sh "$SCRIPT_DIR/gates/test.sh"

echo "Alpha-rooted checker reconstruction OK ($(wc -c < "$OMEGA_PATH_ALPHA_CHECKER/artifacts/proof_checker_bytecode.tape" | tr -d ' ')-byte artifact)"
