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
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 574503 bb0926635a8f6442c2a8b40e8ad015a5db7f2eea914062892d50ee6905104726" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 589552 69aa773f4e006e9a737643cb0f97033c471176aed7dd31429e9cc205a4496e70" \
  "omega-request $OMEGA_PATH_OMEGA_REQUEST_ENTRY 574035 9ee93e13f56ed6937be21e0a24c9469d817a5498806dc0c4c03089e6c98a4bfc" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 571677 3fc101fd95c3e7474b3fe2ccef26e905ad26e144b042e6fbffd6a0aeb0b05ff7" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main_ocreq.epsilon 589169 bb0df0b84735b0a32c27838a187619934cce5d37fa1105033db75137c2791419" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 573551 fcbcda4189d01931ba3e296235d7680257a075136cf3994565161414d455d039" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 573004 452b1864345a935ab7bce378ba6646b37477e6c08e531961d44cb491ca383032" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 572744 5b8b21e7d7e91ce63f7b71cfd1d7122d43ba2d0f6d4184975956b77cf87bcbca" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 572770 e9df055b44309313841e736e5d7631d57ac5587e6573c3f17da74aeea5ab4dc7" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 572717 c12b0fedda175dfa5106bb9c8e38f001c148097d5e27b81ea00bb327cf2b91bb" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 574345 1738a54b1a3d19bf97fdfc3e9bd9d66294426fd0c7bf78e538ff2d1e3784c7cd" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 573047 5bf917e743d3d4a9c3a8846f58ede2b0cd865b4dd5219ad5afb20a24055014e9" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 573113 0c6f6b9bb9d71aaf680b83c8b7220cec44162751adb2d0454df2820395783858" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_i.epsilon 574381 f0fbc43cc70e2221f0166fe7df1c407d317bc1244f8ba5db4033a4d9cb32a7fc"
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

for needle in \
  "$OMEGA_COMPILER_MANIFEST_SHA256" "$OMEGA_COMPILER_PACKED_SHA256" \
  "569,920"
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
for needle in \
  "$(grouped "$OMEGA_REQUEST_ENTRY_SIZE")" "$OMEGA_REQUEST_ENTRY_SHA256" \
  "$(grouped "$OMEGA_REQUEST_FIXTURE_SIZE")" "$OMEGA_REQUEST_FIXTURE_SHA256"
do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-request/README.md" ||
    fail "tests/bootstrap/omega-request/README.md lacks bound record $needle"
done
for needle in "$OMEGA_REQUEST_ENTRY_SIZE" "$OMEGA_REQUEST_ENTRY_SHA256" \
  "$OMEGA_REQUEST_FIXTURE_SIZE" "$OMEGA_REQUEST_FIXTURE_SHA256"; do
  grep -q "$needle" "$OMEGA_REPO_ROOT/tests/bootstrap/omega-request/gate.py" ||
    fail "omega-request gate.py lacks bound record $needle"
done
for OMEGA_EXECUTABLE_ENTRY in \
  "$OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE $OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE $OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256" \
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
