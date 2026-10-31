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
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 584640 acd224c6ae0dc2717d99944ed0f2cf638587487be22f793b8c1d4384acfb4f9c" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 591498 e92e06045e1a092ba2f131344e3ba1add54699616639a0e43db1910200fc430b" \
  "omega-request $OMEGA_PATH_OMEGA_REQUEST_ENTRY 575978 193ee78f2da4c90b1f71001564aa95a4deb5d51055290bdd2d31c91029104d50" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 573623 faa3447737e103451f5d9197c38aa8386f15f48bc37360446ad3d53721732cea" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main_ocreq.epsilon 591115 74d562ec471da3e1d3d10da61e2e732d0304cf367978a5e60b9fd4c7f824ace8" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 575497 1ca2637874b9b535bb55d42bd3b7ed5973a010c907f5b1af0956f7aedfabc707" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 574950 e5cf1393b2ddb69cd52d116d3537921dd57e642378473265d43e8bd98e6a1a07" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 574690 91dbc9d01ffdfe7176030b32a5d68e236a120fe3139846f1bc21c6c23e2fb0dc" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 574716 721b48fa87a43a1bffdd87e6583ed13e6e909262ec2a130be29dc100b8f61180" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 574663 be74d9c52da0b852a782ad21bea86b9f3e6866a412ba1f9054f37b146327a6a8" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 576291 76074ef7631e4ea2fc4bbedbcf6483490c999b06ca22c9f88717990357269fad" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 574993 335521ee764e77e3e4bb473e979ec7d917f494ce5ede41469be8578dbb787920" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 575059 c9436b5fb4234454fa3c9dfe25a3b955a0345f6f071ef831b7b059c12fc5926b" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_i.epsilon 575729 a556840d2eedbf27041b229232409c7efea73020c5622e73b398b5ce96376f26" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_j.epsilon 576327 c8d90e675a538c8f44e16f0be0c39597bec9a9268379e1147db5868013485ad1"
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
