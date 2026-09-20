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
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 562744 eba237c61a2e430be54541d9befe0a8ceb0d854e3ff90ddcc15d49e8f99cd730" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 576391 215059c09aad24799cd9c5013168e994768adbcd4587f5d2314c44d9ea6d2242" \
  "omega-request $OMEGA_PATH_OMEGA_REQUEST_ENTRY 562276 95c875c51ea8f4babca5672f1cc4265c9ad96d469c84dc507ee06e5eaea99b06" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 559918 fcf88cc850822c90a3caaf38a89d8bf594312b1e4ef154bc2ac63cb7234674cc" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main_ocreq.epsilon 577414 0195756cd505588cd1aa2aa22c4f7465fd09c4f5c299c4011bd7a71025b2828c" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 561500 f22dbae93f2b1cc4478fdaf1d7053dd2d9f6405bb1d651fec13a234e884361b8" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 561245 e9d4627c6226ba39359708d5163010499210386dcb51e82577e89e757734a5c1" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 560985 72ef281f99dd95a96287807d3e7d200b412aa831e570a3c290474eec59046d53" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 561011 69fb11609028859b5584d464f1551269c0d0571aa7470e8e9d9bfec789253a82" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 560864 f6d343ba22a3690ae7d42f47f41d0e65f7890dfe5ad7696626abb100d9288e4a" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 562586 e3712a9642ade95f7733307198734da2613d186baf9e02b3ee40378e13178821" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 561288 e9586c29c06358aef499ace1dc3f49144dca86a5bc24707d73b0639ce2f556de" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 561354 c461e4ac6b09b109962148618880186d32ec8a87300befe53ae41814ff2cbdc5"
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
  "558,161"
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
