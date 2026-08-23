#!/usr/bin/env sh
# Focused gate for the first Alpha-written Beta compiler slice.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "bc Alpha cold start: cannot find repository root" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"

ASSEMBLER="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

"$ASSEMBLER" < "$SCRIPT_DIR/bc-alpha.alpha" > "$TMP/bc-alpha.tape"
stamp_seed "$TMP/bc-alpha.tape" "$SEED" "$TMP/bc-alpha" >/dev/null

pass=0
fail=0

accept() {
  name=$1
  source=$2
  expected=$3
  if ! printf '%s\n' "$source" | "$TMP/bc-alpha" > "$TMP/$name.alpha"; then
    set +e
    printf '%s\n' "$source" | "$TMP/bc-alpha" > /dev/null
    status=$?
    set -e
    echo "FAIL $name: compiler rejected valid Slice-A source (status $status)" >&2
    fail=$((fail + 1))
    return
  fi
  if ! "$ASSEMBLER" < "$TMP/$name.alpha" > "$TMP/$name.tape"; then
    echo "FAIL $name: emitted invalid Alpha assembly" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$TMP/$name.tape" "$SEED" "$TMP/$name" >/dev/null
  set +e
  "$TMP/$name" </dev/null
  actual=$?
  set -e
  if [ "$actual" = "$expected" ]; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: expected exit $expected, got $actual" >&2
    fail=$((fail + 1))
  fi
}

reject() {
  name=$1
  source=$2
  set +e
  printf '%s\n' "$source" | "$TMP/bc-alpha" > "$TMP/$name.out"
  status=$?
  set -e
  if [ "$status" -ne 0 ] && [ ! -s "$TMP/$name.out" ]; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: invalid source status=$status output=$(wc -c < "$TMP/$name.out" | tr -d ' ')" >&2
    fail=$((fail + 1))
  fi
}

accept literal 'proc main() { return 42 }' 42
accept precedence 'proc main() { return 2 + 3 * 4 }' 14
accept parentheses 'proc main() { return (2 + 3) * 4 }' 20
accept associativity 'proc main() { return 100 - 58 + 7 % 5 }' 44
accept division 'proc main() { return 100 / 7 }' 14
accept character "proc main() { return 'A' }" 65
accept escaped "proc main() { return '\\n' }" 10
accept comments ' ; before
proc main() { ; body
  return 6 * 7 ; result
}' 42

printf '%s' 'imm r15,1048576
call main
halt r0
main:
imm r0,42
ret
' > "$TMP/literal.expected"
if cmp -s "$TMP/literal.alpha" "$TMP/literal.expected"; then
  pass=$((pass + 1))
else
  echo "FAIL literal_output: complete emitted Alpha stream changed" >&2
  fail=$((fail + 1))
fi

reject missing_expression 'proc main() { return }'
reject trailing_source 'proc main() { return 42 } proc other() { return 0 }'
reject keyword_boundary 'procedure main() { return 42 }'
reject wrong_entry 'proc answer() { return 42 }'
reject decimal_extent 'proc main() { return 1234567890 }'
reject bad_character "proc main() { return '\\x' }"
deep='proc main() { return '
i=0
while [ "$i" -lt 65 ]; do deep="${deep}("; i=$((i + 1)); done
deep="${deep}1"
i=0
while [ "$i" -lt 65 ]; do deep="${deep})"; i=$((i + 1)); done
deep="${deep} }"
reject nesting_extent "$deep"
wide=$(awk 'BEGIN { printf "proc main() { return 1"; for (i = 0; i < 12000; i++) printf "+1"; print " }" }')
reject output_extent "$wide"

# Pin the compiler-owned source extent: exactly 1 MiB is accepted, while the
# next byte is observed before any table write or output publication.
limit_source="$TMP/source-limit.beta"
printf '%s' 'proc main() { return 42 }' > "$limit_source"
used=$(wc -c < "$limit_source" | tr -d ' ')
remaining=$((1048576 - used))
dd if=/dev/zero bs="$remaining" count=1 2>/dev/null >> "$limit_source"
if "$TMP/bc-alpha" < "$limit_source" > "$TMP/source-limit.alpha" &&
   "$ASSEMBLER" < "$TMP/source-limit.alpha" > "$TMP/source-limit.tape"; then
  pass=$((pass + 1))
else
  echo "FAIL source_limit: exact 1 MiB source was not accepted" >&2
  fail=$((fail + 1))
fi
printf '\000' >> "$limit_source"
set +e
"$TMP/bc-alpha" < "$limit_source" > "$TMP/source-over.out"
status=$?
set -e
if [ "$status" -eq 2 ] && [ ! -s "$TMP/source-over.out" ]; then
  pass=$((pass + 1))
else
  echo "FAIL source_over: expected status 2 and empty output, got status $status" >&2
  fail=$((fail + 1))
fi

echo "bc Alpha cold start Slice A: $pass passed, $fail failed ($(wc -c < "$TMP/bc-alpha.tape" | tr -d ' ')-byte compiler tape)"
[ "$fail" -eq 0 ]
