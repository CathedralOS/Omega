#!/usr/bin/env sh
# Focused gate for the complete Beta-written Gamma compiler surface.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Gamma compiler gate: cannot find repository root" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"

SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT
materialize_beta_assembler "$TMP/assembler" >/dev/null
ASSEMBLER="$TMP/assembler"
OUTCOME_CODES="$OMEGA_PATH_GAMMA_COMPILER/outcomes-v1.tsv"

outcome_name() {
  kind=$1
  code=$2
  awk -F '\t' -v kind="$kind" -v code="$code" \
    '$1 == kind && $2 == code { print $3; found = 1 } END { if (!found) exit 1 }' \
    "$OUTCOME_CODES"
}

outcome_coordinate_space() {
  kind=$1
  code=$2
  awk -F '\t' -v kind="$kind" -v code="$code" \
    '$1 == kind && $2 == code { print $4; found = 1 } END { if (!found) exit 1 }' \
    "$OUTCOME_CODES"
}

# Decode one already-staged compiler failure. This consumer enforces the
# canonical carrier; it never repairs or infers missing producer fields.
decode_failure() {
  frame=$1
  halt_tag=$2
  [ "$(wc -c < "$frame" | tr -d ' ')" -eq 40 ] || return 1
  set -- $(od -An -tu1 -v "$frame")
  [ "$#" -eq 40 ] || return 1
  [ "$1" -eq 255 ] && [ "$2" -eq 71 ] && [ "$3" -eq 67 ] &&
    [ "$4" -eq 79 ] && [ "$5" -eq 85 ] && [ "$6" -eq 84 ] &&
    [ "$7" -eq 1 ] && [ "$8" -eq 0 ] || return 1
  shift 8
  decoded_kind=$1
  decoded_space=$2
  [ "$3" -eq 0 ] && [ "$4" -eq 0 ] || return 1
  shift 4
  decoded_code=$(( $1 + ($2 << 8) + ($3 << 16) + ($4 << 24) ))
  shift 4
  decoded_coordinate=$(( $1 + ($2 << 8) + ($3 << 16) + ($4 << 24) + ($5 << 32) + ($6 << 40) + ($7 << 48) + ($8 << 56) ))
  shift 8
  decoded_limit=$(( $1 + ($2 << 8) + ($3 << 16) + ($4 << 24) + ($5 << 32) + ($6 << 40) + ($7 << 48) + ($8 << 56) ))
  shift 8
  decoded_requested=$(( $1 + ($2 << 8) + ($3 << 16) + ($4 << 24) + ($5 << 32) + ($6 << 40) + ($7 << 48) + ($8 << 56) ))
  [ "$decoded_kind" -eq "$halt_tag" ] || return 1
  case "$decoded_kind" in
    1)
      decoded_family=reject
      [ "$decoded_space" -eq 1 ] && [ "$decoded_limit" -eq 0 ] &&
        [ "$decoded_requested" -eq 0 ] || return 1
      ;;
    2)
      decoded_family=incomplete
      [ "$decoded_space" -eq 1 ] || [ "$decoded_space" -eq 2 ] || return 1
      [ "$decoded_requested" -gt "$decoded_limit" ] || return 1
      ;;
    3)
      decoded_family=internal
      [ "$decoded_space" -ge 0 ] && [ "$decoded_space" -le 3 ] &&
        [ "$decoded_limit" -eq 0 ] && [ "$decoded_requested" -eq 0 ] || return 1
      ;;
    *) return 1 ;;
  esac
  decoded_name=$(outcome_name "$decoded_family" "$decoded_code") || return 1
  decoded_space_name=$(outcome_coordinate_space "$decoded_family" "$decoded_code") || return 1
  case "$decoded_space_name" in
    none) expected_space=0 ;;
    source_byte) expected_space=1 ;;
    emitted_payload_byte) expected_space=2 ;;
    internal_row) expected_space=3 ;;
    *) return 1 ;;
  esac
  [ "$decoded_space" -eq "$expected_space" ] || return 1
}

expect_failure() {
  frame=$1
  halt_tag=$2
  expected_name=$3
  expected_limit=$4
  expected_requested=$5
  expected_coordinate=$6
  decode_failure "$frame" "$halt_tag" &&
    [ "$decoded_name" = "$expected_name" ] &&
    [ "$decoded_limit" -eq "$expected_limit" ] &&
    [ "$decoded_requested" -eq "$expected_requested" ] &&
    [ "$decoded_coordinate" -eq "$expected_coordinate" ]
}

"$ASSEMBLER" < "$OMEGA_PATH_GAMMA_COMPILER_SOURCE" > "$TMP/compiler.tape"
stamp_seed "$TMP/compiler.tape" "$SEED" "$TMP/compiler" >/dev/null

pass=0
fail=0

accept() {
  name=$1
  source=$2
  expected=$3
  if ! printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"; then
    set +e
    printf '%s\n' "$source" | "$TMP/compiler" > /dev/null
    status=$?
    set -e
    echo "FAIL $name: compiler rejected valid Gamma source (status $status)" >&2
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
  if ! printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"; then
    echo "FAIL $name: compiler rejected valid I/O source" >&2
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

# Runtime trap identity belongs to Alpha's exact observation gate rather than a
# host shell signal number. Here the compiler contract pins that the exact Gamma
# operation does not halt normally and preserves every preceding output byte.
accept_trap_prefix() {
  name=$1
  source=$2
  expected_output=$3
  if ! printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"; then
    echo "FAIL $name: compiler rejected valid trapping source" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$TMP/$name.tape" "$SEED" "$TMP/$name" >/dev/null
  set +e
  sh -c '"$1" </dev/null > "$2"' sh "$TMP/$name" "$TMP/$name.stdout" 2>/dev/null
  actual_status=$?
  set -e
  printf '%b' "$expected_output" > "$TMP/$name.expected"
  if [ "$actual_status" -eq 132 ] && cmp -s "$TMP/$name.stdout" "$TMP/$name.expected"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: expected a trap after bytes $(od -An -tu1 "$TMP/$name.expected"), got status $actual_status and $(od -An -tu1 "$TMP/$name.stdout")" >&2
    fail=$((fail + 1))
  fi
}

reject() {
  name=$1
  source=$2
  expected_coordinate=$3
  set +e
  printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.out"
  status=$?
  set -e
  if [ "$status" -eq 1 ] && expect_failure "$TMP/$name.out" "$status" invalid_source 0 0 "$expected_coordinate"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: invalid source did not return canonical Reject (status=$status, output=$(wc -c < "$TMP/$name.out" | tr -d ' '))" >&2
    fail=$((fail + 1))
  fi
}

reject_file() {
  name=$1
  source_file=$2
  expected_coordinate=$3
  set +e
  "$TMP/compiler" < "$source_file" > "$TMP/$name.out"
  status=$?
  set -e
  if [ "$status" -eq 1 ] && expect_failure "$TMP/$name.out" "$status" \
      invalid_source 0 0 "$expected_coordinate"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: invalid source byte did not return canonical Reject at $expected_coordinate (status=$status)" >&2
    fail=$((fail + 1))
  fi
}

incomplete() {
  name=$1
  resource=$2
  limit=$3
  requested=$4
  source=$5
  expected_coordinate=$6
  set +e
  printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.out"
  status=$?
  set -e
  if [ "$status" -eq 2 ] && expect_failure "$TMP/$name.out" "$status" "$resource" "$limit" "$requested" "$expected_coordinate"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: resource refusal did not return canonical Incomplete/$resource (status=$status)" >&2
    fail=$((fail + 1))
  fi
}

internal_mutant() {
  name=$1
  reason=$2
  needle=$3
  occurrence=$4
  replacement=$5
  source=$6
  expected_coordinate=$7
  mutant_source="$TMP/$name.beta"
  mutant_tape="$TMP/$name.compiler.tape"
  mutant_executable="$TMP/$name.compiler"
  if ! awk -v needle="$needle" -v occurrence="$occurrence" \
      -v replacement="$replacement" '
        $0 == needle {
          seen++
          if (seen == occurrence) { print replacement; changed++; next }
        }
        { print }
        END { if (changed != 1) exit 1 }
      ' "$OMEGA_PATH_GAMMA_COMPILER_SOURCE" > "$mutant_source" ||
     ! "$ASSEMBLER" < "$mutant_source" > "$mutant_tape"; then
    echo "FAIL $name: could not construct the single-site compiler mutant" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$mutant_tape" "$SEED" "$mutant_executable" >/dev/null
  set +e
  printf '%s\n' "$source" | "$mutant_executable" > "$TMP/$name.out"
  status=$?
  set -e
  if [ "$status" -eq 3 ] && expect_failure "$TMP/$name.out" "$status" \
      "$reason" 0 0 "$expected_coordinate"; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: did not return canonical InternalFailure/$reason (status=$status)" >&2
    fail=$((fail + 1))
  fi
}

# Compile a valid source without running its artifact and pin the exact private
# tape extent. This is used at the maximum runnable Alpha payload, where
# execution would only add noise to the compiler-capacity check.
accept_compile_extent() {
  name=$1
  source=$2
  expected_bytes=$3
  set +e
  printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"
  status=$?
  set -e
  actual_bytes=$(wc -c < "$TMP/$name.tape" | tr -d ' ')
  if [ "$status" -eq 0 ] && [ "$actual_bytes" -eq "$expected_bytes" ]; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: expected accepted $expected_bytes-byte tape, got status $status and $actual_bytes bytes" >&2
    fail=$((fail + 1))
  fi
}

# A memory-invalid run is outside the admitted Gamma profile, but the generated
# tape must contain it as runtime status 251 before any physical tape/stack
# access rather than aliasing another Alpha region.
contain_memory_fault() {
  name=$1
  source=$2
  if ! printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"; then
    echo "FAIL $name: compiler rejected memory-containment source" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$TMP/$name.tape" "$SEED" "$TMP/$name" >/dev/null
  set +e
  "$TMP/$name" </dev/null > "$TMP/$name.stdout"
  status=$?
  set -e
  if [ "$status" -eq 251 ] && [ ! -s "$TMP/$name.stdout" ]; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: invalid memory escaped containment (status $status)" >&2
    fail=$((fail + 1))
  fi
}

contain_stack_fault() {
  name=$1
  source=$2
  if ! printf '%s\n' "$source" | "$TMP/compiler" > "$TMP/$name.tape"; then
    echo "FAIL $name: compiler rejected stack-containment source" >&2
    fail=$((fail + 1))
    return
  fi
  stamp_seed "$TMP/$name.tape" "$SEED" "$TMP/$name" >/dev/null
  set +e
  "$TMP/$name" </dev/null > "$TMP/$name.stdout"
  status=$?
  set -e
  if [ "$status" -eq 250 ] && [ ! -s "$TMP/$name.stdout" ]; then
    pass=$((pass + 1))
  else
    echo "FAIL $name: stack exhaustion was not contained (status $status)" >&2
    fail=$((fail + 1))
  fi
}

accept literal 'proc main() { return 42 }' 42
accept full_word 'proc main() { return 18446744073709551615 }' 255
accept full_word_wrap 'proc main() { return 18446744073709551615 + 1 }' 0
accept full_word_high_byte 'proc main() { word[0] = 18446744073709551615 return byte[7] }' 255
accept full_word_signed 'proc main() { return 18446744073709551615 < 0 }' 1
accept leading_zero_word 'proc main() { return 00000000000000000000000000042 }' 42
accept precedence 'proc main() { return 2 + 3 * 4 }' 14
accept parentheses 'proc main() { return (2 + 3) * 4 }' 20
accept associativity 'proc main() { return 100 - 58 + 7 % 5 }' 44
accept division 'proc main() { return 100 / 7 }' 14
accept signed_division 'proc main() { return (0 - 7) / 2 }' 253
accept signed_remainder 'proc main() { return (0 - 7) % 2 }' 255
accept signed_division_both_negative 'proc main() { return (0 - 7) / (0 - 2) }' 3
accept signed_remainder_negative_divisor 'proc main() { return 7 % (0 - 2) }' 1
accept signed_remainder_both_negative 'proc main() { return (0 - 7) % (0 - 2) }' 255
accept_trap_prefix division_by_zero_prefix 'proc main() { emit("D") return 1 / 0 }' D
accept_trap_prefix remainder_by_zero_prefix 'proc main() { emit("R") return 1 % 0 }' R
accept_trap_prefix signed_division_overflow_prefix 'proc main() { emit("O") return 9223372036854775808 / 18446744073709551615 }' O
accept_trap_prefix signed_remainder_overflow_prefix 'proc main() { emit("M") return 9223372036854775808 % 18446744073709551615 }' M
accept_trap_prefix rhs_trap_order 'proc mark(x) { write_byte(x) return x } proc main() { emit("X") return mark(65) + 1 / 0 }' XA
accept character "proc main() { return 'A' }" 65
accept escaped "proc main() { return '\\n' }" 10
accept comments ' ; before
proc main() { ; body
  return 6 * 7 ; result
}' 42
cr_comments=$(printf '; before\rproc main() { ; body\r return 42\r}')
accept cr_comments "$cr_comments" 42
unset cr_comments
printf '; hidden\000\nproc main() { return 0 }\n' > "$TMP/comment-nul.gamma"
reject_file comment_nul "$TMP/comment-nul.gamma" 8
printf 'proc\013main() { return 0 }\n' > "$TMP/vertical-tab.gamma"
reject_file vertical_tab "$TMP/vertical-tab.gamma" 4
printf '; hidden\177\nproc main() { return 0 }\n' > "$TMP/comment-del.gamma"
reject_file comment_del "$TMP/comment-del.gamma" 8
printf '; hidden\303\251\nproc main() { return 0 }\n' > "$TMP/comment-high.gamma"
reject_file comment_high "$TMP/comment-high.gamma" 8
accept locals 'proc main() { let a = 6 let b = 7 return a * b }' 42
accept assignment 'proc main() { let x = 10 x = x + 32 return x }' 42
accept forward_call 'proc main() { return double(21) } proc double(x) { return x + x }' 42
accept nested_calls 'proc main() { return add(mul(2, 3), 4) } proc add(a, b) { return a + b } proc mul(a, b) { return a * b }' 10
accept factorial_recursion 'proc main() { return fact(5) } proc fact(n) { state r { to b when n < 2 return n * fact(n - 1) } state b { return 1 } }' 120
accept fibonacci_recursion 'proc main() { return fib(10) } proc fib(n) { state r { to b when n < 2 return fib(n - 1) + fib(n - 2) } state b { return n } }' 55
stack_source='proc dive(n) {'
stack_slot=0
while [ "$stack_slot" -lt 63 ]; do
  stack_source="$stack_source let v$stack_slot = $stack_slot"
  stack_slot=$((stack_slot + 1))
done
stack_source="$stack_source return dive(n + 1) } proc main() { return dive(0) }"
contain_stack_fault recursive_stack_exhaustion "$stack_source"
unset stack_source stack_slot
accept call_state_loop 'proc main() { return sumto(10) } proc sumto(n) { let t = 0 let i = 1 state l { to b when i <= n return t } state b { t = t + i i = i + 1 to l } }' 55
accept four_parameters 'proc sum(a, b, c, d) { let x = c * 10 + d return a + b + x } proc main() { return sum(1, 2, 3, 9) }' 42
accept_io nested_four_argument_order 'proc mark(x) { write_byte(x + 48) return x } proc pack(a, b, c, d) { return a + b + c + d } proc main() { return pack(mark(1), mark(2), mark(3), mark(4)) }' '' 10 1234
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
accept nested_state_dfs_fallthrough 'proc main() { state outer { state child { let x = 42 } } state next { return x } }' 42
accept alternate_path_initialization 'proc main() { to assigned when read_byte() state initialize { let x = 1 to join } state assigned { x = 2 to join } state join { return x } }' 2
accept unreachable_initialized_read 'proc main() { return 7 state declared { let x = 1 } state dead { return x } }' 7
accept scoped_states 'proc main() { return f() * 10 + g() } proc f() { state same { return 4 } } proc g() { state same { return 2 } }' 42
accept shared_state_spelling 'proc main() { let shared = 1 state shared { return 42 } }' 42
accept adversarial_labels 'proc main() { state main { return _L0() } } proc _L0() { state foo__bar { return 42 } } proc foo__bar() { return 0 }' 42
accept byte_memory 'proc main() { let b = 2097152 byte[b] = 65 byte[b + 1] = 66 return byte[b] + byte[b + 1] }' 131
accept word_memory 'proc main() { let b = 2097152 word[b] = 42 return word[b] }' 42
accept byte_into_unaligned_word_alias 'proc main() { word[1] = 0 byte[3] = 255 return word[1] / 65536 }' 255
accept unaligned_word_from_bytes 'proc main() { byte[7] = 17 byte[8] = 34 return word[7] / 256 }' 34
accept zeroed_byte_memory 'proc main() { return byte[0] }' 0
accept zeroed_word_memory 'proc main() { return word[0] }' 0
accept last_byte_memory 'proc main() { byte[134217727] = 77 return byte[134217727] }' 77
accept last_word_memory 'proc main() { word[134217720] = 42 return word[134217720] }' 42
contain_memory_fault byte_memory_over 'proc main() { return byte[134217728] }'
contain_memory_fault word_memory_over 'proc main() { return word[134217721] }'
contain_memory_fault negative_memory_address 'proc main() { byte[18446744073709551615] = 1 return 0 }'
accept word_array_loop 'proc main() { let base = 2097152 let i = 0 state fill { to fb when i < 5 to sum_init } state fb { word[base + i * 8] = i * i i = i + 1 to fill } state sum_init { let t = 0 i = 0 to sum } state sum { to sb when i < 5 return t } state sb { t = t + word[base + i * 8] i = i + 1 to sum } }' 30
accept nested_memory 'proc main() { let b = 2097152 word[b] = b + 16 byte[b + 16] = 77 return byte[word[b]] }' 77
accept call_statement 'proc main() { let b = 2097152 touch(b) return word[b] } proc touch(p) { word[p] = 42 return 0 }' 42
accept final_fallthrough_zero 'proc main() { return f(42) } proc f(x) { }' 0
accept final_state_fallthrough_zero 'proc main() { state last { } }' 0
accept explicit_return_preserved 'proc main() { return f(42) } proc f(x) { return x }' 42
accept_io byte_io 'proc main() { let c = read_byte() write_byte(c + 1) return c }' A 65 B
accept_io eof_read 'proc main() { return read_byte() }' '' 255 ''
accept_io write_return 'proc main() { return write_byte(321) }' '' 65 A
accept_io exact_eof_word 'proc main() { return read_byte() == 18446744073709551615 }' '' 1 ''
accept_io exact_write_return_word 'proc main() { return write_byte(256) / 256 }' '' 1 '\0'
accept_io binary_evaluation_order 'proc mark(x) { write_byte(x + 48) return x } proc main() { return mark(3) + mark(4) }' '' 7 34
accept_io call_argument_evaluation_order 'proc mark(x) { write_byte(x + 48) return x } proc pair(a, b) { return a * 10 + b } proc main() { return pair(mark(1), mark(2)) }' '' 12 12
accept_io store_evaluation_order 'proc address() { write_byte(65) return 0 } proc value() { write_byte(86) return 42 } proc main() { word[address()] = value() return word[0] }' '' 42 AV
accept_io emit_text 'proc main() { emit("A\n") return 42 }' '' 42 'A\n'
accept_io emit_empty 'proc main() { emit("") return 7 }' '' 7 ''

if cmp -s "$TMP/guarded_true.tape" "$TMP/guarded_grouped.tape"; then
  pass=$((pass + 1))
else
  echo "FAIL guard_parentheses: optional grouping changed emitted Alpha" >&2
  fail=$((fail + 1))
fi

printf '%s' 'imm r15,2097152
imm r14,2097152
imm r13,8
call main
halt r0
main:
imm r5,8
sub r15,r5
imm r4,1048576
jlt r15,r4,stack_fault
store r15,r14
mov r14,r15
imm r0,42
mov r15,r14
load r14,r15
add r15,r13
ret
imm r0,0

mov r15,r14
load r14,r15
add r15,r13
ret
stack_fault:
imm r4,250
halt r4
' | "$ASSEMBLER" > "$TMP/literal.expected.tape"
if cmp -s "$TMP/literal.tape" "$TMP/literal.expected.tape"; then
  pass=$((pass + 1))
else
  echo "FAIL literal_output: complete emitted Alpha tape changed" >&2
  fail=$((fail + 1))
fi

reject missing_expression 'proc main() { return }' 21
reject malformed_late_proc 'proc main() { return 42 } proc other(' 38
reject keyword_boundary 'procedure main() { return 42 }' 0
reject wrong_entry 'proc answer() { return 42 }' 28
reject parameterized_main 'proc main(x) { return x }' 26
reject duplicate_proc 'proc main() { return 1 } proc main() { return 2 }' 36
reject duplicate_parameter 'proc main() { return f(1, 2) } proc f(x, x) { return x }' 42
reject duplicate_local 'proc main() { let x = 1 let x = 2 return x }' 34
reject reserved_local 'proc main() { let return = 1 return 0 }' 29
reject reserved_read_proc 'proc read_byte() { return 1 } proc main() { return 0 }' 16
reject reserved_write_proc 'proc write_byte(x) { return x } proc main() { return 0 }' 18
reject reserved_read_local 'proc main() { let read_byte = 1 return read_byte }' 32
reject reserved_write_local 'proc main() { let write_byte = 1 return write_byte }' 33
reject reserved_read_state 'proc main() { state read_byte { return 0 } }' 29
reject reserved_write_state 'proc main() { state write_byte { return 0 } }' 30
reject unknown_variable 'proc main() { return x }' 23
reject unknown_assignment 'proc main() { x = 1 return x }' 16
reject unknown_call 'proc main() { return nope() }' 30
reject arity_mismatch 'proc main() { return f(1) } proc f(a, b) { return a + b }' 58
reject five_parameters 'proc main() { return 0 } proc f(a, b, c, d, e) { return a }' 45
reject five_arguments 'proc main() { return f(1, 2, 3, 4, 5) } proc f(a, b, c, d) { return a }' 36
reject single_equal_expression 'proc main() { return 1 = 1 }' 24
reject single_bang_expression 'proc main() { return 1 ! 2 }' 24
reject chained_comparison 'proc main() { return 1 < 2 < 3 }' 27
reject missing_parameter_comma 'proc f(a b) { return a } proc main() { return f(1, 2) }' 9
reject trailing_parameter_comma 'proc f(a,) { return a } proc main() { return f(1) }' 9
reject missing_argument_comma 'proc f(a, b) { return a } proc main() { return f(1 2) }' 51
reject trailing_argument_comma 'proc f(a) { return a } proc main() { return f(1,) }' 48
reject split_less_equal 'proc main() { return 1 < = 2 }' 25
reject unknown_state 'proc main() { to nowhere }' 27
reject duplicate_state 'proc main() { state x { return 1 } state x { return 2 } }' 42
reject cross_proc_state 'proc main() { to x } proc f() { state x { return 1 } }' 55
reject reserved_state 'proc main() { state state { return 0 } }' 25
reject read_arity 'proc main() { return read_byte(1) }' 33
reject write_arity_zero 'proc main() { return write_byte() }' 33
reject write_arity_two 'proc main() { return write_byte(1, 2) }' 37
reject bad_memory_load 'proc main() { return byte[1 }' 28
reject bad_memory_store 'proc main() { word[1] 42 return 0 }' 22
reject unterminated_emit 'proc main() { emit("unterminated) return 0 }' 45
reject bad_emit_escape 'proc main() { emit("bad\x") return 0 }' 24
reject bad_emit_single_quote_escape "proc main() { emit(\"bad\\'\") return 0 }" 24
reject decimal_overflow 'proc main() { return 18446744073709551616 }' 21
reject bad_character "proc main() { return '\\x' }" 21
reject bad_character_double_quote_escape "proc main() { return '\\\"' }" 21
reject ordinary_after_state 'proc main() { state child { } return 0 }' 30
reject ordinary_after_nested_state 'proc main() { state child { state nested { } return 0 } }' 45
reject ordinary_after_return 'proc main() { return 0 let x = 1 }' 23
reject ordinary_after_unconditional_transition 'proc main() { to done let x = 1 state done { return 0 } }' 22
reject skipped_initialization 'proc main() { to bypass when read_byte() state initialize { let x = 1 to join } state bypass { to join } state join { return x } }' 131
reject traversal_order_initialization 'proc main() { to head state initialize { let x = 1 to head } state head { to initialize when read_byte() return x } }' 118
reject self_initializer 'proc main() { let x = x return 0 }' 24
reject assignment_before_declaration 'proc main() { x = 1 let x = 2 return x }' 16

reject_noncanonical_frame() {
  name=$1
  frame=$2
  halt_tag=$3
  if decode_failure "$frame" "$halt_tag"; then
    echo "FAIL $name: noncanonical compiler observation decoded" >&2
    fail=$((fail + 1))
  else
    pass=$((pass + 1))
  fi
}

# The observation consumer is fail-closed: it stages output and admits neither
# malformed diagnostics nor partial/trapping producer output as an artifact.
dd if="$TMP/missing_expression.out" of="$TMP/frame-truncated" bs=39 count=1 2>/dev/null
reject_noncanonical_frame diagnostic_truncated "$TMP/frame-truncated" 1
cp "$TMP/missing_expression.out" "$TMP/frame-unknown-code"
printf '\177' | dd of="$TMP/frame-unknown-code" bs=1 seek=12 conv=notrunc 2>/dev/null
reject_noncanonical_frame diagnostic_unknown_code "$TMP/frame-unknown-code" 1
cp "$TMP/missing_expression.out" "$TMP/frame-reserved"
printf '\001' | dd of="$TMP/frame-reserved" bs=1 seek=10 conv=notrunc 2>/dev/null
reject_noncanonical_frame diagnostic_reserved_field "$TMP/frame-reserved" 1
cp "$TMP/missing_expression.out" "$TMP/frame-coordinate-space"
printf '\002' | dd of="$TMP/frame-coordinate-space" bs=1 seek=9 conv=notrunc 2>/dev/null
reject_noncanonical_frame diagnostic_coordinate_space "$TMP/frame-coordinate-space" 1
cp "$TMP/missing_expression.out" "$TMP/frame-kind-mismatch"
printf '\002' | dd of="$TMP/frame-kind-mismatch" bs=1 seek=8 conv=notrunc 2>/dev/null
reject_noncanonical_frame diagnostic_halt_disagreement "$TMP/frame-kind-mismatch" 1
cp "$TMP/missing_expression.out" "$TMP/frame-unknown-version"
printf '\002' | dd of="$TMP/frame-unknown-version" bs=1 seek=6 conv=notrunc 2>/dev/null
reject_noncanonical_frame diagnostic_unknown_version "$TMP/frame-unknown-version" 1
cp "$TMP/missing_expression.out" "$TMP/frame-partial-output"
printf '\000' >> "$TMP/frame-partial-output"
reject_noncanonical_frame diagnostic_partial_output "$TMP/frame-partial-output" 1
: > "$TMP/frame-trap"
reject_noncanonical_frame diagnostic_trap "$TMP/frame-trap" 132
reject_noncanonical_frame runtime_250_not_compiler_outcome "$TMP/missing_expression.out" 250
reject_noncanonical_frame runtime_251_not_compiler_outcome "$TMP/missing_expression.out" 251

# Pin each source-shape table at its last admitted row and first refused row.
# These are private compiler ceilings and every refusal returns the exact
# version-1 Incomplete carrier without publishing artifact bytes.
ident_64=$(awk 'BEGIN { for (i = 0; i < 64; i++) printf "a" }')
ident_65="${ident_64}a"
accept identifier_limit "proc main() { let $ident_64 = 42 return $ident_64 }" 42
incomplete identifier_extent identifier_bytes 64 65 \
  "proc main() { let $ident_65 = 1 return 0 }" 82

deep_64='proc main() { return '
i=0
while [ "$i" -lt 64 ]; do deep_64="${deep_64}("; i=$((i + 1)); done
deep_64="${deep_64}1"
i=0
while [ "$i" -lt 64 ]; do deep_64="${deep_64})"; i=$((i + 1)); done
deep_64="${deep_64} }"
accept nesting_limit "$deep_64" 1
deep_65=$(printf '%s' "$deep_64" | sed 's/return /return (/; s/ }$/) }/')
incomplete nesting_extent syntax_depth 64 65 "$deep_65" 86

nested_calls_64='proc main() { return '
i=0
while [ "$i" -lt 64 ]; do nested_calls_64="${nested_calls_64}identity("; i=$((i + 1)); done
nested_calls_64="${nested_calls_64}1"
i=0
while [ "$i" -lt 64 ]; do nested_calls_64="${nested_calls_64})"; i=$((i + 1)); done
nested_calls_64="${nested_calls_64} } proc identity(x) { return x }"
accept call_nesting_limit "$nested_calls_64" 1
nested_calls_65=$(printf '%s' "$nested_calls_64" | sed 's/return /return identity(/; s/ } proc identity/) } proc identity/')
incomplete call_nesting_extent syntax_depth 64 65 "$nested_calls_65" 605

deep_load_64='proc main() { return '
i=0
while [ "$i" -lt 64 ]; do deep_load_64="${deep_load_64}word["; i=$((i + 1)); done
deep_load_64="${deep_load_64}2097152"
i=0
while [ "$i" -lt 64 ]; do deep_load_64="${deep_load_64}]"; i=$((i + 1)); done
deep_load_64="${deep_load_64} }"
accept load_nesting_limit "$deep_load_64" 0
deep_load_65=$(printf '%s' "$deep_load_64" | sed 's/return /return word[/; s/ }$/] }/')
incomplete load_nesting_extent syntax_depth 64 65 "$deep_load_65" 345

nested_states_64='proc main() {'
i=0
while [ "$i" -lt 64 ]; do nested_states_64="${nested_states_64} state s${i} {"; i=$((i + 1)); done
nested_states_64="${nested_states_64} return 1"
i=0
while [ "$i" -lt 64 ]; do nested_states_64="${nested_states_64} }"; i=$((i + 1)); done
nested_states_64="${nested_states_64} }"
accept state_nesting_limit "$nested_states_64" 1
nested_states_65=$(printf '%s' "$nested_states_64" | sed 's/ return 1/ state overflow { return 1 }/')
incomplete state_nesting_extent syntax_depth 64 65 "$nested_states_65" 788

mixed_state_depth_64='proc main() { state nested { return '
i=0
while [ "$i" -lt 63 ]; do mixed_state_depth_64="${mixed_state_depth_64}("; i=$((i + 1)); done
mixed_state_depth_64="${mixed_state_depth_64}1"
i=0
while [ "$i" -lt 63 ]; do mixed_state_depth_64="${mixed_state_depth_64})"; i=$((i + 1)); done
mixed_state_depth_64="${mixed_state_depth_64} } }"
accept mixed_state_nesting_limit "$mixed_state_depth_64" 1
mixed_state_depth_65=$(printf '%s' "$mixed_state_depth_64" | sed 's/return /return (/; s/ } }$/) } }/')
incomplete mixed_state_nesting_extent syntax_depth 64 65 "$mixed_state_depth_65" 100

# A fixed emit body lets the gate hit AlphaBootstrapV2's 1,048,572-byte
# runnable payload exactly. The second source requests 1,048,573 bytes and
# must publish nothing.
tape_limit=$(awk 'BEGIN { printf "proc main() { emit(\""; for (i = 0; i < 87365; i++) printf "a"; print "\") return 1 + 1 }" }')
accept_compile_extent output_limit "$tape_limit" 1048572
if sh "$OMEGA_PATH_GAMMA_COMPILER/validation/admission/gc-artifact-structure.sh" \
    "$TMP/output_limit.tape" >/dev/null; then
  pass=$((pass + 1))
else
  echo "FAIL output_limit_structure: maximum-size emitted tape is malformed" >&2
  fail=$((fail + 1))
fi
tape_over=$(awk 'BEGIN { printf "proc main() { emit(\""; for (i = 0; i < 87370; i++) printf "a"; print "\") return 0 }" }')
incomplete output_extent payload_bytes 1048572 1048573 "$tape_over" 1048572
incomplete first_payload_failure_before_late_reject payload_bytes 1048572 1048573 \
  "$tape_over proc broken(" 1048572
reject first_reject_before_late_payload "proc main() { return } $tape_over" 21

wide=$(awk 'BEGIN { printf "proc main() { return 1"; for (i = 0; i < 56000; i++) printf "+1"; print " }" }')
set +e
printf '%s\n' "$wide" | "$TMP/compiler" > "$TMP/expression_output_extent.out"
status=$?
set -e
if [ "$status" -eq 2 ] && decode_failure "$TMP/expression_output_extent.out" "$status" &&
   [ "$decoded_name" = payload_bytes ] && [ "$decoded_limit" -eq 1048572 ] &&
   [ "$decoded_requested" -gt 1048572 ] && [ "$decoded_coordinate" -eq 1048572 ]; then
  pass=$((pass + 1))
else
  echo "FAIL expression_output_extent: did not return canonical payload Incomplete" >&2
  fail=$((fail + 1))
fi
slots_64=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 64; i++) printf " let v%d = %d", i, i; print " return v63 }" }')
accept slot_limit "$slots_64" 63
slots_65=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 65; i++) printf " let v%d = %d", i, i; print " return 0 }" }')
incomplete slot_extent frame_slot_rows 64 65 "$slots_65" 839
calls_1024=$(awk 'BEGIN { printf "proc main() { return zero()"; for (i = 1; i < 1024; i++) printf " + zero()"; print " } proc zero() { return 0 }" }')
accept call_global_limit "$calls_1024" 0
calls_1025=$(awk 'BEGIN { printf "proc main() { return zero()"; for (i = 1; i < 1025; i++) printf " + zero()"; print " } proc zero() { return 0 }" }')
incomplete call_global_extent call_rows 1024 1025 "$calls_1025" 9243
procs_256=$(awk 'BEGIN { print "proc main() { return 0 }"; for (i = 0; i < 255; i++) printf "proc p%d() { return %d }\n", i, i }')
accept procedure_limit "$procs_256" 0
procs_257="$procs_256 proc overflow() { return 0 }"
incomplete procedure_extent procedure_rows 256 257 "$procs_257" 6705
states_128=$(awk 'BEGIN { printf "proc main() { return 0"; for (i = 0; i < 128; i++) printf " state s%d { }", i; print " }" }')
accept state_proc_limit "$states_128" 0
states_129=$(awk 'BEGIN { printf "proc main() { return 0"; for (i = 0; i < 129; i++) printf " state s%d { }", i; print " }" }')
incomplete state_proc_extent procedure_state_rows 128 129 "$states_129" 1843
states_1024=$(awk 'BEGIN { for (p = 0; p < 8; p++) { if (p == 0) printf "proc main() { return 0"; else printf "proc p%d() { return 0", p; for (i = 0; i < 128; i++) printf " state s%d { }", i; print " }" } }')
accept state_global_limit "$states_1024" 0
states_1025="$states_1024 proc extra() { state overflow { return 0 } }"
incomplete state_global_extent global_state_rows 1024 1025 "$states_1025" 14695
edges_256=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 256; i++) printf " to done when 0"; print " state done { return 0 } }" }')
accept edge_proc_limit "$edges_256" 0
edges_257=$(awk 'BEGIN { printf "proc main() {"; for (i = 0; i < 257; i++) printf " to done when 0"; print " state done { return 0 } }" }')
incomplete edge_proc_extent procedure_edge_rows 256 257 "$edges_257" 3861
edges_1024=$(awk 'BEGIN { for (p = 0; p < 4; p++) { if (p == 0) printf "proc main() {"; else printf "proc p%d() {", p; for (i = 0; i < 256; i++) printf " to done when 0"; print " state done { return 0 } }" } }')
accept edge_global_limit "$edges_1024" 0
edges_1025="$edges_1024 proc extra_edges() { to done when 0 state done { return 0 } }"
incomplete edge_global_extent global_edge_rows 1024 1025 "$edges_1025" 15542

# The 116508-fixup and 262144-internal-label arrays are secondary corruption
# guards rather than independently reachable source-profile limits. A fixup
# requires at least one 9-byte direct-reference instruction, and each internal
# identity requires at least four emitted control bytes, so the exact tape
# ceiling above is necessarily reached first.

# Pin the compiler-owned source extent: exactly 1 MiB is accepted, while the
# next byte is observed before any table write or output publication.
limit_source="$TMP/source-limit.gamma"
printf '%s' 'proc main() { return 42 }' > "$limit_source"
used=$(wc -c < "$limit_source" | tr -d ' ')
remaining=$((1048576 - used))
dd if=/dev/zero bs="$remaining" count=1 2>/dev/null | tr '\000' ' ' >> "$limit_source"
if "$TMP/compiler" < "$limit_source" > "$TMP/source-limit.tape" &&
   cmp -s "$TMP/source-limit.tape" "$TMP/literal.tape"; then
  pass=$((pass + 1))
else
  echo "FAIL source_limit: exact 1 MiB source was not accepted as the unchanged program" >&2
  fail=$((fail + 1))
fi
printf ' ' >> "$limit_source"
set +e
"$TMP/compiler" < "$limit_source" > "$TMP/source-over.out"
status=$?
set -e
if [ "$status" -eq 2 ] && expect_failure "$TMP/source-over.out" "$status" \
    source_bytes 1048576 1048577 1048576; then
  pass=$((pass + 1))
else
  echo "FAIL source_over: expected canonical source-bytes Incomplete, got status $status" >&2
  fail=$((fail + 1))
fi

# Source admission completes before parsing, so an overlong stream is the first
# decisive event even when its first byte would later be invalid source.
printf '!' | dd of="$limit_source" bs=1 seek=0 conv=notrunc 2>/dev/null
set +e
"$TMP/compiler" < "$limit_source" > "$TMP/source-over-invalid.out"
status=$?
set -e
if [ "$status" -eq 2 ] && expect_failure "$TMP/source-over-invalid.out" "$status" \
    source_bytes 1048576 1048577 1048576; then
  pass=$((pass + 1))
else
  echo "FAIL source_over_precedence: slurp refusal did not remain the first outcome" >&2
  fail=$((fail + 1))
fi

# Every closed InternalFailure reason has a positive producer test. The
# production capacities remain dominant; each temporary compiler lowers one
# otherwise dominated invariant and must publish the exact canonical frame.
internal_source='proc main() { return 0 }'
internal_mutant internal_replay_rejected replay_rejected \
  '        jnz   r0, internal_error' 1 '        jmp   internal_error' "$internal_source" 25
internal_mutant internal_replay_length_mismatch replay_length_mismatch \
  '        jeq   r1, r2, source_patch' 1 '        jlt   r1, r1, source_patch' "$internal_source" 133
internal_mutant internal_replay_payload_overflow replay_payload_overflow \
  '        jeq   r1, r2, eb_error' 1 '        jmp   eb_error' "$internal_source" 0
internal_mutant internal_fixup_capacity fixup_capacity \
  '        jeq   r7, r1, af_error' 1 '        jmp   af_error' "$internal_source" 0
internal_mutant internal_pc_capacity internal_pc_capacity \
  '        jlt   r6, r1, rip_store' 1 '        jmp   rip_error' "$internal_source" 0
internal_mutant internal_post_validation_resolution post_validation_resolution \
  '        jlt   r6, r1, rpc_store' 1 '        jmp   rpc_error' "$internal_source" 0

echo "Gamma compiler complete surface: $pass passed, $fail failed ($(wc -c < "$TMP/compiler.tape" | tr -d ' ')-byte compiler tape)"
[ "$fail" -eq 0 ]
