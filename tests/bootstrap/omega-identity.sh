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
  "omega-parser $OMEGA_PATH_OMEGA_PARSER_ENTRY 566377 ed3cd51c8bbfff315eeeada926cd131d419179fa090f4a80fafda7d782512693" \
  "omega-outcome $OMEGA_PATH_OMEGA_OUTCOME_ENTRY 581426 c0fc0f0abc69130b8e7549db419c0fda7be2fbbea92ef5ffc52a00adf64a0435" \
  "omega-request $OMEGA_PATH_OMEGA_REQUEST_ENTRY 565909 804093474370a707a29982b05a2eb38bfb899f74beca2053803843054dcb8df6" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main.epsilon 563551 fe6429f9607ac796ff4e0eacc4a93308775555982279ffec9d454e10b4262523" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/main_ocreq.epsilon 581043 a0db24def35174efb5bd19815f569bc97b34365eb35c4958a62422f36d2ad6d8" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls.epsilon 565425 19723460df970ba0b93b7bfcab8552cca4f6a1ac308ecc747b624db449fa19d4" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_b.epsilon 564878 9c8d4f04677ac112a7166f1f9901a7b02d197af5301651b1d6852e6a89ad5d47" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_c.epsilon 564618 e53add24891b8f00c632f36c2ee989e6464478571a213458a005f37aaf8332cc" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_d.epsilon 564644 d301154c9b067bd93686ae39f235457e7a8e588a1aeb826b4794fc73bf44183e" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_e.epsilon 564591 a6e9efa4622d87253f904a6e4e2166232cc5361375ebbf2f98a967e1f2515286" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_f.epsilon 566219 b1d0bd4a7608367c56f2473d86d45541b7471ac799bff3e2438b54a2bf5db460" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_g.epsilon 564921 89c1c23ef356f3875db54645cbccfe17ea9977dc80814013d0eee4da7e07cbf0" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_h.epsilon 564987 a409990e78c521a20aff59ccfbb69666318a35ec983edf953c581b036135877b" \
  "omega-executable $OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/controls_i.epsilon 565657 67bcbc3a8b734d81f84870b2544b0ee079dc09d65de9cb7fbc766470bcab9c8f"
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
  "561,794"
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
  "$OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256" \
  "$OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256"
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
