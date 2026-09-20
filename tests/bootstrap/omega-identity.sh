#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Omega identity: skipped (python3 absent)"
  exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

flip_byte() {
  if [ "$(od -An -tc -j 100 -N1 "$1" | tr -d ' ')" = "a" ]; then
    printf 'b' | dd of="$1" bs=1 seek=100 conv=notrunc status=none
  else
    printf 'a' | dd of="$1" bs=1 seek=100 conv=notrunc status=none
  fi
}

require_omega_compiler_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: bound manifest and packed closure pass"

require_omega_parser_entry_identity ||
  fail "bound parser entry refused the canonical checkout"
require_omega_outcome_entry_identity ||
  fail "bound outcome entry refused the canonical checkout"
require_omega_executable_entries_identity ||
  fail "bound executable entries refused the canonical checkout"
echo "entries: bound gate-local customer entries pass"

materialize_omega_compiler "$TMP/compiler.epsilon" ||
  fail "materialization of the bound closure failed"
require_bound_identity "materialized compiler" "$TMP/compiler.epsilon" \
  "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256" \
  "bootstrap/5_omega/README.md" ||
  fail "materialized closure differs from the bound identity"
echo "materialize: packed compiler is exactly the bound member bytes"

# Packed customers: the materialized compiler plus each bound gate-local
# entry is the customer byte sequence its gate consumes; each is bound at
# its recorded identity.
for OMEGA_PACKED_CUSTOMER in \
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 562648 a47e7628d0b2a59f7a83c5b68dd3f367eb8a910e1bf4926c7f66be01489810de" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 576295 517bee1ce9b180a993eb1a90e6d32e997afb08e8d4ba8c945d7637ceb355b919" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 559824 c3b86f71eba1e0f7317c9c4cd1b3afc8d26e2d80a1e114322c84beb20234548d" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 561404 a30e4e007367669e90e6356407eb0647552ec4415947b34629c4ce8601c3865a" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 561149 ebbab1f132eafc5d3910d7ca7b90f1d7280caad9bad9c6d0f2f0b818bf559748" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 560889 dcca542508d8a5c8406b0cbeacb53ef9f2d17c5d43cc16a0c3d5aecefa17ef8a" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 560915 9ea9b0abca75b330fbbde43fc40e801a85a9fca184f79324849ff94d79c71e23" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 560768 a62270323e17dea5fef4da69fb2042c1c690a5cbb36ca471cf5d7709a6c84ed1" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 562490 be615c94a9f7812e33743580758e4bc66b2439f2e267dcec35ad68c285ce8822" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 561192 3d2ebf05e91b6357d971acf291d8f04dec2e96444f17365339869ce787d4554a" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 561258 4414e832300e9cc352a02b53e01baf1d1bb8cc3dd7dbf7d342e968a27b8f9e0e"
do
  set -- $OMEGA_PACKED_CUSTOMER
  cat "$TMP/compiler.epsilon" "$2" > "$TMP/customer.epsilon"
  require_bound_identity "packed $1 customer" "$TMP/customer.epsilon" \
    "$3" "$4" "tests/bootstrap/$1/README.md" ||
    fail "packed $1 customer differs from the bound record"
done
echo "customers: materialized compiler plus each bound entry is the recorded customer bytes"

cp -R "$OMEGA_PATH_OMEGA_D" "$TMP/5_omega"
head -c $((OMEGA_COMPILER_MANIFEST_SIZE - 1)) \
  "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" > "$TMP/5_omega/omega_compiler.epsilon.sources"
rc=0
(
  export OMEGA_PATH_OMEGA_COMPILER_SOURCES=$TMP/5_omega/omega_compiler.epsilon.sources
  materialize_omega_compiler "$TMP/refused-truncated"
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated manifest: expected exit 3, got $rc"
[ ! -e "$TMP/refused-truncated" ] ||
  fail "truncated manifest: destination was written"
echo "truncate: a truncated manifest is refused before packing"

cp "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$TMP/5_omega/omega_compiler.epsilon.sources"
CORRUPT_MEMBER="$TMP/5_omega/representations.epsilon"
flip_byte "$CORRUPT_MEMBER"
rc=0
(
  export OMEGA_PATH_OMEGA_COMPILER_SOURCES=$TMP/5_omega/omega_compiler.epsilon.sources
  materialize_omega_compiler "$TMP/refused-member"
) 2>"$TMP/corrupt-member.err" || rc=$?
[ "$rc" != 0 ] ||
  fail "corrupted member: materialization unexpectedly succeeded"
grep -q 'member digest changed' "$TMP/corrupt-member.err" ||
  fail "corrupted member: refusal did not name the member digest"
[ ! -e "$TMP/refused-member" ] ||
  fail "corrupted member: destination was written"
echo "member: a one-byte member change is refused during packing"

cp "$OMEGA_PATH_OMEGA_PARSER_ENTRY" "$TMP/corrupt-parser-entry"
flip_byte "$TMP/corrupt-parser-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_PARSER_ENTRY=$TMP/corrupt-parser-entry
  require_omega_parser_entry_identity
) 2>"$TMP/corrupt-parser-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted parser entry: expected exit 3, got $rc"
grep -q 'omega-parser/README.md' "$TMP/corrupt-parser-entry.err" ||
  fail "corrupted parser entry: refusal did not cite the gate record"
head -c $((OMEGA_PARSER_ENTRY_SIZE - 1)) \
  "$OMEGA_PATH_OMEGA_PARSER_ENTRY" > "$TMP/truncated-parser-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_PARSER_ENTRY=$TMP/truncated-parser-entry
  require_omega_parser_entry_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated parser entry: expected exit 3, got $rc"
echo "parser entry: a one-byte change or truncation is refused with its record"

cp "$OMEGA_PATH_OMEGA_OUTCOME_ENTRY" "$TMP/corrupt-outcome-entry"
flip_byte "$TMP/corrupt-outcome-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_OUTCOME_ENTRY=$TMP/corrupt-outcome-entry
  require_omega_outcome_entry_identity
) 2>"$TMP/corrupt-outcome-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted outcome entry: expected exit 3, got $rc"
grep -q 'omega-outcome/README.md' "$TMP/corrupt-outcome-entry.err" ||
  fail "corrupted outcome entry: refusal did not cite the gate record"
head -c $((OMEGA_OUTCOME_ENTRY_SIZE - 1)) \
  "$OMEGA_PATH_OMEGA_OUTCOME_ENTRY" > "$TMP/truncated-outcome-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_OUTCOME_ENTRY=$TMP/truncated-outcome-entry
  require_omega_outcome_entry_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated outcome entry: expected exit 3, got $rc"
echo "outcome entry: a one-byte change or truncation is refused with its record"

mkdir "$TMP/executable-entries"
cp "$OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES"/*.epsilon "$TMP/executable-entries/"
flip_byte "$TMP/executable-entries/controls_d.epsilon"
rc=0
(
  export OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES=$TMP/executable-entries
  require_omega_executable_entries_identity
) 2>"$TMP/corrupt-executable-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted executable entry: expected exit 3, got $rc"
grep -q 'omega-executable/README.md' "$TMP/corrupt-executable-entry.err" ||
  fail "corrupted executable entry: refusal did not cite the gate record"
cp "$OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon" \
  "$TMP/executable-entries/controls_d.epsilon"
head -c $((OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE - 1)) \
  "$OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon" \
  > "$TMP/executable-entries/main.epsilon"
rc=0
(
  export OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES=$TMP/executable-entries
  require_omega_executable_entries_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated executable entry: expected exit 3, got $rc"
echo "executable entries: a one-byte change or truncation is refused with its record"

for needle in \
  "$OMEGA_COMPILER_MANIFEST_SHA256" "$OMEGA_COMPILER_PACKED_SHA256" \
  "558,065"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/bootstrap/5_omega/README.md" ||
    fail "bootstrap/5_omega/README.md lacks bound record $needle"
done
for record in \
  tests/bootstrap/omega-parser/gate.py \
  tests/bootstrap/omega-outcome/gate.py \
  tests/bootstrap/source-closure.py
do
  for needle in "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256"
  do
    grep -q "$needle" "$OMEGA_REPO_ROOT/$record" ||
      fail "$record lacks bound record $needle"
  done
done
echo "records: bound identities match bootstrap/5_omega/README.md and the omega-parser, omega-outcome, and source-closure gates"

# Gate READMEs spell byte counts with digit grouping; gate.py pins are raw.
grouped() {
  echo "$1" | sed -e :a -e 's/\(.*[0-9]\)\([0-9][0-9][0-9]\)/\1,\2/;ta'
}

for needle in \
  "$(grouped "$OMEGA_PARSER_ENTRY_SIZE")" "$OMEGA_PARSER_ENTRY_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-parser/README.md" ||
    fail "tests/bootstrap/omega-parser/README.md lacks bound entry record $needle"
done
for needle in "$OMEGA_PARSER_ENTRY_SIZE" "$OMEGA_PARSER_ENTRY_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-parser/gate.py" ||
    fail "omega-parser gate.py lacks bound entry record $needle"
done
for needle in \
  "$(grouped "$OMEGA_OUTCOME_ENTRY_SIZE")" "$OMEGA_OUTCOME_ENTRY_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-outcome/README.md" ||
    fail "tests/bootstrap/omega-outcome/README.md lacks bound entry record $needle"
done
for needle in "$OMEGA_OUTCOME_ENTRY_SIZE" "$OMEGA_OUTCOME_ENTRY_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-outcome/gate.py" ||
    fail "omega-outcome gate.py lacks bound entry record $needle"
done
for OMEGA_EXECUTABLE_ENTRY in \
  "$OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE $OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256"
do
  set -- $OMEGA_EXECUTABLE_ENTRY
  grep -q "$(grouped "$1")" \
    "$OMEGA_REPO_ROOT/tests/bootstrap/omega-executable/README.md" ||
    fail "tests/bootstrap/omega-executable/README.md lacks bound entry size $1"
  grep -q "$2" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-executable/README.md" ||
    fail "tests/bootstrap/omega-executable/README.md lacks bound entry digest $2"
done
echo "entries: bound entry pins agree with the omega-* READMEs and gate.py pins"

echo "Omega identity: bound closure materialized exactly; corrupted manifest, member, and gate-local entries refused"
