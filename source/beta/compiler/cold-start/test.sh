#!/usr/bin/env sh
# Focused gate for the complete Alpha-written Beta compiler surface.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "bc Alpha cold start: cannot find repository root" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
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
    echo "FAIL $name: compiler rejected valid Beta source (status $status)" >&2
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

accept_io() {
  name=$1
  source=$2
  input=$3
  expected_status=$4
  expected_output=$5
  if ! printf '%s\n' "$source" | "$TMP/bc-alpha" > "$TMP/$name.alpha" ||
     ! "$ASSEMBLER" < "$TMP/$name.alpha" > "$TMP/$name.tape"; then
    echo "FAIL $name: compiler or assembler rejected valid I/O source" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$TMP/$name.tape" "$SEED" "$TMP/$name" >/dev/null
  set +e
  printf '%s' "$input" | "$TMP/$name" > "$TMP/$name.stdout"
  actual_status=$?
  set -e
  printf '%b' "$expected_output" > "$TMP/$name.expected"
  if [ "$actual_status" = "$expected_status" ] && cmp -s "$TMP/$name.stdout" "$TMP/$name.expected"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: expected status $expected_status and output bytes $(od -An -tu1 "$TMP/$name.expected"), got status $actual_status and $(od -An -tu1 "$TMP/$name.stdout")" >&2
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
accept locals 'proc main() { let a = 6 let b = 7 return a * b }' 42
accept assignment 'proc main() { let x = 10 x = x + 32 return x }' 42
accept forward_call 'proc main() { return double(21) } proc double(x) { return x + x }' 42
accept nested_calls 'proc main() { return add(mul(2, 3), 4) } proc add(a, b) { return a + b } proc mul(a, b) { return a * b }' 10
accept four_parameters 'proc sum(a, b, c, d) { let x = c * 10 + d return a + b + x } proc main() { return sum(1, 2, 3, 9) }' 42
accept less_true 'proc main() { return 1 < 2 }' 1
accept less_false 'proc main() { return 2 < 1 }' 0
accept greater_true 'proc main() { return 2 > 1 }' 1
accept greater_false 'proc main() { return 2 > 2 }' 0
accept equal_true 'proc main() { return 2 == 2 }' 1
accept equal_false 'proc main() { return 2 == 3 }' 0
accept not_equal_true 'proc main() { return 2 != 3 }' 1
accept not_equal_false 'proc main() { return 2 != 2 }' 0
accept less_equal_true 'proc main() { return 2 <= 2 }' 1
accept less_equal_false 'proc main() { return 3 <= 2 }' 0
accept greater_equal_true 'proc main() { return 3 >= 2 }' 1
accept greater_equal_false 'proc main() { return 2 >= 3 }' 0
accept signed_comparison 'proc main() { return (0 - 1) < 0 }' 1
accept comparison_precedence 'proc main() { return 1 + 2 * 3 == 7 }' 1
accept nested_comparison 'proc main() { return 1 < (2 == 2) }' 0
accept nested_comparison_left 'proc main() { return (1 < 2) == 1 }' 1
accept state_jump 'proc main() { state start { to done } state done { return 42 } }' 42
accept guarded_true 'proc main() { let a = 5 state start { to yes when a < 10 return 0 } state yes { return 42 } }' 42
accept guarded_grouped 'proc main() { let a = 5 state start { to yes when (a < 10) return 0 } state yes { return 42 } }' 42
accept guarded_false 'proc main() { let a = 15 state start { to yes when a < 10 return 7 } state yes { return 42 } }' 7
accept state_loop 'proc main() { let n = 10 let s = 0 let i = 1 state loop { to body when i <= n return s } state body { s = s + i i = i + 1 to loop } }' 55
accept scoped_states 'proc main() { return f() } proc f() { state same { return 42 } } proc g() { state same { return 1 } }' 42
accept shared_state_spelling 'proc main() { let shared = 1 state shared { return 42 } }' 42
accept adversarial_labels 'proc main() { state main { return _L0() } } proc _L0() { state foo__bar { return 42 } } proc foo__bar() { return 0 }' 42
accept byte_memory 'proc main() { let b = 2097152 byte[b] = 65 byte[b + 1] = 66 return byte[b] + byte[b + 1] }' 131
accept word_memory 'proc main() { let b = 2097152 word[b] = 42 return word[b] }' 42
accept nested_memory 'proc main() { let b = 2097152 word[b] = b + 16 byte[b + 16] = 77 return byte[word[b]] }' 77
accept call_statement 'proc main() { let b = 2097152 touch(b) return word[b] } proc touch(p) { word[p] = 42 return 0 }' 42
accept_io byte_io 'proc main() { let c = read_byte() write_byte(c + 1) return c }' A 65 B
accept_io emit_text 'proc main() { emit("A\n") return 42 }' '' 42 'A\n'
accept_io emit_empty 'proc main() { emit("") return 7 }' '' 7 ''

if cmp -s "$TMP/guarded_true.alpha" "$TMP/guarded_grouped.alpha"; then
  pass=$((pass + 1))
else
  echo "FAIL guard_parentheses: optional grouping changed emitted Alpha" >&2
  fail=$((fail + 1))
fi

printf '%s' 'imm r15,1048576
imm r14,1048576
call main
halt r0
main:
imm r5,8
sub r15,r5
store r15,r14
mov r14,r15
imm r0,42
mov r15,r14
load r14,r15
imm r2,8
add r15,r2
ret

mov r15,r14
load r14,r15
imm r2,8
add r15,r2
ret
' > "$TMP/literal.expected"
if cmp -s "$TMP/literal.alpha" "$TMP/literal.expected"; then
  pass=$((pass + 1))
else
  echo "FAIL literal_output: complete emitted Alpha stream changed" >&2
  fail=$((fail + 1))
fi

reject missing_expression 'proc main() { return }'
reject malformed_late_proc 'proc main() { return 42 } proc other('
reject keyword_boundary 'procedure main() { return 42 }'
reject wrong_entry 'proc answer() { return 42 }'
reject parameterized_main 'proc main(x) { return x }'
reject duplicate_proc 'proc main() { return 1 } proc main() { return 2 }'
reject duplicate_parameter 'proc main() { return f(1, 2) } proc f(x, x) { return x }'
reject duplicate_local 'proc main() { let x = 1 let x = 2 return x }'
reject reserved_local 'proc main() { let return = 1 return 0 }'
reject unknown_variable 'proc main() { return x }'
reject unknown_assignment 'proc main() { x = 1 return x }'
reject unknown_call 'proc main() { return nope() }'
reject arity_mismatch 'proc main() { return f(1) } proc f(a, b) { return a + b }'
reject five_parameters 'proc main() { return 0 } proc f(a, b, c, d, e) { return a }'
reject five_arguments 'proc main() { return f(1, 2, 3, 4, 5) } proc f(a, b, c, d) { return a }'
reject single_equal_expression 'proc main() { return 1 = 1 }'
reject single_bang_expression 'proc main() { return 1 ! 2 }'
reject chained_comparison 'proc main() { return 1 < 2 < 3 }'
reject split_less_equal 'proc main() { return 1 < = 2 }'
reject unknown_state 'proc main() { to nowhere return 0 }'
reject duplicate_state 'proc main() { state x { return 1 } state x { return 2 } }'
reject cross_proc_state 'proc main() { to x return 0 } proc f() { state x { return 1 } }'
reject reserved_state 'proc main() { state state { return 0 } }'
reject read_arity 'proc main() { return read_byte(1) }'
reject write_arity_zero 'proc main() { return write_byte() }'
reject write_arity_two 'proc main() { return write_byte(1, 2) }'
reject bad_memory_load 'proc main() { return byte[1 }'
reject bad_memory_store 'proc main() { word[1] 42 return 0 }'
reject unterminated_emit 'proc main() { emit("unterminated) return 0 }'
reject bad_emit_escape 'proc main() { emit("bad\x") return 0 }'
reject decimal_extent 'proc main() { return 1234567890 }'
reject bad_character "proc main() { return '\\x' }"
long_ident=$(awk 'BEGIN { for (i = 0; i < 65; i++) printf "a" }')
reject identifier_extent "proc main() { let $long_ident = 1 return 0 }"
deep='proc main() { return '
i=0
while [ "$i" -lt 65 ]; do deep="${deep}("; i=$((i + 1)); done
deep="${deep}1"
i=0
while [ "$i" -lt 65 ]; do deep="${deep})"; i=$((i + 1)); done
deep="${deep} }"
reject nesting_extent "$deep"
deep_load='proc main() { return '
i=0
while [ "$i" -lt 65 ]; do deep_load="${deep_load}word["; i=$((i + 1)); done
deep_load="${deep_load}2097152"
i=0
while [ "$i" -lt 65 ]; do deep_load="${deep_load}]"; i=$((i + 1)); done
deep_load="${deep_load} }"
reject load_nesting_extent "$deep_load"
wide=$(awk 'BEGIN { printf "proc main() { return 1"; for (i = 0; i < 12000; i++) printf "+1"; print " }" }')
reject output_extent "$wide"
many_slots=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 65; i++) printf " let v%d = %d", i, i; print " return 0 }" }')
reject slot_extent "$many_slots"
calls_1024=$(awk 'BEGIN { printf "proc main() { return zero()"; for (i = 1; i < 1024; i++) printf " + zero()"; print " } proc zero() { return 0 }" }')
accept call_global_limit "$calls_1024" 0
calls_1025=$(awk 'BEGIN { printf "proc main() { return zero()"; for (i = 1; i < 1025; i++) printf " + zero()"; print " } proc zero() { return 0 }" }')
reject call_global_extent "$calls_1025"
many_procs=$(awk 'BEGIN { print "proc main() { return 0 }"; for (i = 0; i < 128; i++) printf "proc p%d() { return %d }\n", i, i }')
reject procedure_extent "$many_procs"
states_128=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 128; i++) printf " state s%d { }", i; print " return 0 }" }')
accept state_proc_limit "$states_128" 0
states_129=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 129; i++) printf " state s%d { }", i; print " return 0 }" }')
reject state_proc_extent "$states_129"
states_1024=$(awk 'BEGIN { for (p = 0; p < 8; p++) { if (p == 0) printf "proc main() {"; else printf "proc p%d() {", p; for (i = 0; i < 128; i++) printf " state s%d { }", i; print " return 0 }" } }')
accept state_global_limit "$states_1024" 0
states_1025="$states_1024 proc extra() { state overflow { return 0 } }"
reject state_global_extent "$states_1025"
edges_256=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 256; i++) printf " to done"; print " state done { return 0 } }" }')
accept edge_proc_limit "$edges_256" 0
edges_257=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 257; i++) printf " to done"; print " state done { return 0 } }" }')
reject edge_proc_extent "$edges_257"
edges_1024=$(awk 'BEGIN { for (p = 0; p < 4; p++) { if (p == 0) printf "proc main() {"; else printf "proc p%d() {", p; for (i = 0; i < 256; i++) printf " to done"; print " state done { return 0 } }" } }')
accept edge_global_limit "$edges_1024" 0
edges_1025="$edges_1024 proc extra_edges() { to done state done { return 0 } }"
reject edge_global_extent "$edges_1025"

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

echo "bc Alpha cold start complete surface: $pass passed, $fail failed ($(wc -c < "$TMP/bc-alpha.tape" | tr -d ' ')-byte compiler tape)"
[ "$fail" -eq 0 ]
