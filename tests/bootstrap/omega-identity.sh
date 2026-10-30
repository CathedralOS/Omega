#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_ENVS="omega/compiler_env.sh"
. "$TEST_DIR/gate_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Omega identity: skipped (python3 absent)"
  exit 0
}

. "$TEST_DIR/gate_tmp.sh"

require_omega_compiler_identity ||
  fail "bound identity check refused the canonical checkout"
echo "canonical: bound manifest and packed closure pass"

require_omega_parser_entry_identity ||
  fail "bound parser entry refused the canonical checkout"
require_omega_outcome_entry_identity ||
  fail "bound outcome entry refused the canonical checkout"
require_omega_request_entry_identity ||
  fail "bound request entry refused the canonical checkout"
require_omega_request_fixture_identity ||
  fail "bound request fixture refused the canonical checkout"
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
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 582785 3ceb640563ef2ce57dd3089bbc117a7345813b06dbf800acf18419339d1b1364" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 591550 3c30afc1e7acd977777420e7f5cf51a9d852830316c9f045dec88648a724180e" \
  "omega-request $OMEGA_PATH_OMEGA_REQUEST_ENTRY 576030 e9d854c774fe60c5893b7d5175dedf4a39338eb328c982e57067740a66a3277f" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 573675 fff47baac0707431232ba82308eae0d18a871577420b1792d3bfd87976d6772e" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main_ocreq.epsilon 591167 f6518a83962ab34b1910bf9eeed066c589ddb7d9b49c7895eb76cb298ffb5690" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 575549 eaaebb0770cc77b5e4c0a36a9b63c8595d1119ef0aefc769669c889590652c08" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 575002 b9568aa46572c819f47a0d798eecd27f8eb7b480142f93f47cd753856ada5572" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 574742 82d9b18a2d05c10204c23c383c495346bd18946102f6c7467c665a25f7d91494" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 574768 2e64fcf840373613b653e4837fe64d3c021cb5abca8325f8a3fa24d084eaa1da" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 574715 8e8528338cf326875a7f02a97d6d76b80450270d95fc264c15ef952ef0a96023" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 576343 8bcfa76847d2d2c64d35a3c8ef347bcb183f524ac02bb5b2e03b2da8c64142e1" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 575045 fb5d159d4ee11f21aa11706fe937034f4481ac230560966343ede967ea20928e" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 575111 13a4c9f7575daf3905c255592408b8017f40a6446f7010f961a6f162bf41e178" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_i.epsilon 575781 78171b3b4946a95b0690e3a69dbfe34dd3382f8fd348d56416a4c0ed20cbcc48" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_j.epsilon 576379 7ddd3da28634a7bbf149b241b799c122e667d29ce475ccc94ab5ffbdec05c275"
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

cp "$OMEGA_PATH_OMEGA_REQUEST_ENTRY" "$TMP/corrupt-request-entry"
flip_byte "$TMP/corrupt-request-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_REQUEST_ENTRY=$TMP/corrupt-request-entry
  require_omega_request_entry_identity
) 2>"$TMP/corrupt-request-entry.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted request entry: expected exit 3, got $rc"
grep -q 'omega-request/README.md' "$TMP/corrupt-request-entry.err" ||
  fail "corrupted request entry: refusal did not cite the gate record"
head -c $((OMEGA_REQUEST_ENTRY_SIZE - 1)) \
  "$OMEGA_PATH_OMEGA_REQUEST_ENTRY" > "$TMP/truncated-request-entry"
rc=0
(
  export OMEGA_PATH_OMEGA_REQUEST_ENTRY=$TMP/truncated-request-entry
  require_omega_request_entry_identity
) 2>/dev/null || rc=$?
[ "$rc" = 3 ] ||
  fail "truncated request entry: expected exit 3, got $rc"
cp "$OMEGA_PATH_OMEGA_REQUEST_FIXTURE" "$TMP/corrupt-request-fixture"
flip_byte "$TMP/corrupt-request-fixture"
rc=0
(
  export OMEGA_PATH_OMEGA_REQUEST_FIXTURE=$TMP/corrupt-request-fixture
  require_omega_request_fixture_identity
) 2>"$TMP/corrupt-request-fixture.err" || rc=$?
[ "$rc" = 3 ] ||
  fail "corrupted request fixture: expected exit 3, got $rc"
grep -q 'omega-request/README.md' "$TMP/corrupt-request-fixture.err" ||
  fail "corrupted request fixture: refusal did not cite the gate record"
echo "request entry: a one-byte change or truncation is refused with its record"

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

for record in \
  tests/bootstrap/omega-parser/gate.py \
  tests/bootstrap/gate_lib.py \
  tests/bootstrap/source-closure.py \
  tests/epsilon/d-composition/customers.py
do
  for needle in "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256"
  do
    grep -q "$needle" "$OMEGA_REPO_ROOT/$record" ||
      fail "$record lacks bound record $needle"
  done
done
echo "records: bound identities match every inline packed-record pin"

for needle in "$OMEGA_PARSER_ENTRY_SIZE" "$OMEGA_PARSER_ENTRY_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-parser/gate.py" ||
    fail "omega-parser gate.py lacks bound entry record $needle"
done
for needle in "$OMEGA_OUTCOME_ENTRY_SIZE" "$OMEGA_OUTCOME_ENTRY_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-outcome/gate.py" ||
    fail "omega-outcome gate.py lacks bound entry record $needle"
done
for needle in "$OMEGA_REQUEST_ENTRY_SIZE" "$OMEGA_REQUEST_ENTRY_SHA256" \
  "$OMEGA_REQUEST_FIXTURE_SIZE" "$OMEGA_REQUEST_FIXTURE_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-request/gate.py" ||
    fail "omega-request gate.py lacks bound record $needle"
done
echo "entries: bound entry pins agree with the gate.py pins"

echo "Omega identity: bound closure materialized exactly; corrupted manifest, member, and gate-local entries refused"
