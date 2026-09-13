#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Proofs identity: skipped (python3 absent)"
  exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

require_derivation_checker_identity ||
  fail "bound checker identity check refused the canonical checkout"
require_beta_encoding_theory_identity ||
  fail "bound theory identity check refused the canonical checkout"
echo "canonical: bound manifests and packed member closures pass"

materialize_derivation_checker "$TMP/checker.gamma" ||
  fail "materialization of the bound checker closure failed"
require_bound_identity "materialized checker" "$TMP/checker.gamma" \
  "$DERIVATION_CHECKER_PACKED_SIZE" "$DERIVATION_CHECKER_PACKED_SHA256" \
  "bootstrap/proofs/checker/README.md" ||
  fail "materialized checker closure differs from the bound identity"
materialize_beta_encoding_theory "$TMP/theory.gamma" ||
  fail "materialization of the bound theory closure failed"
require_bound_identity "materialized theory" "$TMP/theory.gamma" \
  "$BETA_ENCODING_PACKED_SIZE" "$BETA_ENCODING_PACKED_SHA256" \
  "bootstrap/proofs/beta_encoding/README.md" ||
  fail "materialized theory closure differs from the bound identity"
echo "materialize: packed member closures are exactly the bound bytes"

cp -R "$OMEGA_REPO_ROOT/bootstrap/proofs/checker/implementation" \
  "$TMP/checker-implementation"
head -c $((DERIVATION_CHECKER_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
  > "$TMP/checker-implementation/implementation.gamma.sources"
rc=0
(
  export OMEGA_PATH_DERIVATION_CHECKER_SOURCES=$TMP/checker-implementation/implementation.gamma.sources
  materialize_derivation_checker "$TMP/refused-checker-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated checker manifest: expected exit 3, got $rc"
[ ! -e "$TMP/refused-checker-truncated" ] ||
  fail "truncated checker manifest: destination was written"
echo "truncate: a truncated checker manifest is refused before packing"

cp "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
  "$TMP/checker-implementation/implementation.gamma.sources"
CORRUPT_MEMBER="$TMP/checker-implementation/layout.gamma"
if [ "$(od -An -tc -j 100 -N1 "$CORRUPT_MEMBER" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_DERIVATION_CHECKER_SOURCES=$TMP/checker-implementation/implementation.gamma.sources
  materialize_derivation_checker "$TMP/refused-checker-member"
) 2>"$TMP/corrupt-checker-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted checker member: materialization unexpectedly succeeded"
grep -q 'member digest changed' "$TMP/corrupt-checker-member.err" ||
  fail "corrupted checker member: refusal did not name the member digest"
[ ! -e "$TMP/refused-checker-member" ] ||
  fail "corrupted checker member: destination was written"
echo "member: a one-byte checker member change is refused during packing"

cp -R "$OMEGA_REPO_ROOT/bootstrap/proofs/beta_encoding/theory" "$TMP/theory"
head -c $((BETA_ENCODING_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_BETA_ENCODING_SOURCES" > "$TMP/theory/theory.gamma.sources"
rc=0
(
  export OMEGA_PATH_BETA_ENCODING_SOURCES=$TMP/theory/theory.gamma.sources
  materialize_beta_encoding_theory "$TMP/refused-theory-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated theory manifest: expected exit 3, got $rc"
[ ! -e "$TMP/refused-theory-truncated" ] ||
  fail "truncated theory manifest: destination was written"
echo "truncate: a truncated theory manifest is refused before packing"

cp "$OMEGA_PATH_BETA_ENCODING_SOURCES" "$TMP/theory/theory.gamma.sources"
CORRUPT_MEMBER="$TMP/theory/definitions/counters/bytes.gamma"
if [ "$(od -An -tc -j 100 -N1 "$CORRUPT_MEMBER" | tr -d ' ')" = "a" ]; then
  printf 'b' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
else
  printf 'a' | dd of="$CORRUPT_MEMBER" bs=1 seek=100 conv=notrunc status=none
fi
rc=0
(
  export OMEGA_PATH_BETA_ENCODING_SOURCES=$TMP/theory/theory.gamma.sources
  materialize_beta_encoding_theory "$TMP/refused-theory-member"
) 2>"$TMP/corrupt-theory-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted theory member: materialization unexpectedly succeeded"
grep -q 'member digest changed' "$TMP/corrupt-theory-member.err" ||
  fail "corrupted theory member: refusal did not name the member digest"
[ ! -e "$TMP/refused-theory-member" ] ||
  fail "corrupted theory member: destination was written"
echo "member: a one-byte theory member change is refused during packing"

for needle in \
  "$DERIVATION_CHECKER_MANIFEST_SHA256" "$DERIVATION_CHECKER_PACKED_SHA256" \
  "62,349"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/proofs/checker/README.md" ||
    fail "bootstrap/proofs/checker/README.md lacks bound record $needle"
done
for needle in \
  "$BETA_ENCODING_MANIFEST_SHA256" "$BETA_ENCODING_PACKED_SHA256" \
  "21,305"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/proofs/beta_encoding/README.md" ||
    fail "bootstrap/proofs/beta_encoding/README.md lacks bound record $needle"
done
echo "records: bound identities match the checker and Beta-encoding READMEs"

echo "Proofs identity: bound closures materialized exactly; corrupted manifests and members refused"
