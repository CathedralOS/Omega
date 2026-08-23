#!/usr/bin/env sh
# OMEGA MEANING (slice 0) — real Omega sample programs, run down the RUST-FREE meaning route.
#
# The lattice's meaning-by-elaboration route (decision D2) now reaches the SUMMIT rung: an Omega
# program is translated to gamma by `omega2gamma.beta` (which understands the shared
# delta/omega machine surface — dotted field paths `self.state.n`, subjectless transitions,
# `state name(&mut self)` headers, state-body lets) and EXECUTED by `gamma/interp.beta`. Both are
# Rust-free in steady execution (alpha->beta->bc); the bc cold-start refinement
# remains a separate open edge. Each sample's exit code must equal the "Expected exit: N"
# its header documents — the language's stated intent for that program.
#
# omega (the Rust reference producer) is NOT in this loop; it remains the untrusted fast
# compiler this meaning is one day checked against (translation validation, decision D3). The
# subset grows exactly as omega2gamma's surface grows; samples outside it simply aren't listed.
# Needs no cargo/clang — only bc. No `set -e`: exit codes are data here.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_BETA}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null || { echo "omega-meaning FAIL — Beta compiler artifact"; exit 1; }
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b "${OMEGA_PATH_OMEGA_BOOTSTRAP}/meaning/omega2gamma.beta" "$T/omega2gamma.exe" \
  || { echo "omega-meaning FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta      "$T/interp.exe" || { echo "omega-meaning FAIL — build interp.beta"; exit 1; }

PASS=0; FAIL=0
# om SAMPLE : run samples/SAMPLE/main.omg down the meaning route; exit must equal the documented
# "Expected exit: N". Most samples verify themselves internally and exit a distinguished success
# code only when their own checks pass — so agreement is computation, not coincidence.
om() {
  src="${OMEGA_PATH_CORPUS}/$1/main.omg"
  want=$(grep -oE 'Expected exit: [0-9]+' "$src" | head -1 | grep -oE '[0-9]+')
  [ -n "$want" ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : no documented exit"; return; }
  # The ordinary meaning sweep supplies empty standard input explicitly. Leaving
  # the frontend's STDIN placeholder as a free Gamma identifier made this case
  # depend accidentally on parser name-table order. input-tv.sh substitutes the
  # documented non-empty vectors separately.
  "$T/omega2gamma.exe" < "$src" 2>/dev/null | sed 's/STDIN/Nil/' | "$T/interp.exe" > "$T/mo.out" 2>&1; got=$?
  case "$(head -c 6 "$T/mo.out")" in '(Pair ')                    # dual-channel: exit rides the pair
    got=$(head -1 "$T/mo.out" | sed 's/^(Pair \([0-9]*\) .*/\1/');; esac
  if [ "$got" = "$want" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : meaning-route exit $got, documented $want"; fi
}
# omt NAME : like om but for a local feature test under tests/ (constructs no committed sample
# isolates — e.g. cross-data method calls without floats/struct-arrays alongside).
omt() {
  src="${OMEGA_PATH_OMEGA_BOOTSTRAP}/gates/tests/$1.omg"
  want=$(grep -oE 'Expected exit: [0-9]+' "$src" | head -1 | grep -oE '[0-9]+')
  "$T/omega2gamma.exe" < "$src" 2>/dev/null | "$T/interp.exe" > "$T/mo.out" 2>&1; got=$?
  case "$(head -c 6 "$T/mo.out")" in '(Pair ')                    # dual-channel: exit rides the pair
    got=$(head -1 "$T/mo.out" | sed 's/^(Pair \([0-9]*\) .*/\1/');; esac
  if [ "$got" = "$want" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL tests/$1 : meaning-route exit $got, documented $want"; fi
}

# NB the set is AUDITED, not just swept: a sample whose pass depends on a mis-parse coincidence is
# excluded even if its exit matches (format_number: string buffers). alarm_probe2's old exclusion
# (case-pattern dispatch mis-parse) is RESOLVED: case data v0 lowers no-payload case values to integer
# tags with real per-tag dispatch (verified: distinct tags, zero unbound variables; payload-case
# construction refuses loudly, so payload binders can never observe garbage). Output-mode programs return (Pair <exit> <stdout list>) — BOTH observables; the gate
# parses the exit from the printed pair. width_mixer stays: its `as i32` casts drop harmlessly (widening is a no-op over gamma's
# unbounded ints while values stay in range — the same status as `in Trapping` annotations).
om bank_ledger               # 70 — self-verified (state args)
om bouncing_ball             # 70 — self-verified (state args)
om bounded_counter           # 70 — saturation check stays in range; state args report(70)
om calculator_rpn            # 70 — self-verified (arrays + state args)
om cli_mvp                   # 0  — dual-channel (Pair 0 stdout)
om alarm_probe               # 70 — case dispatch v0: no-payload Trigger arms fire, fire_count reaches 2
om alarm_probe2              # 70 — case dispatch v0 (was audit-excluded as a mis-parse coincidence; now faithful)
om event_log                  # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om token_interpreter          # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om nested_case_payload        # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om score_tracker              # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om stopwatch                  # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om task_runner                # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om elevator                   # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om logger                     # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om alarm_scheduler            # 70 — case payloads v1 ((Pair tag payload), match destructuring)
om vending_machine            # 70 — value arms (`_ -> N` returns the literal)
om calculator                 # 46 — value arms (`_ -> N` returns the literal)
om direction_command          # 1 — value arms (`_ -> N` returns the literal)
om parse_number               # 25 — value arms (`_ -> N` returns the literal)
om shape_area                 # 42 — value arms (`_ -> N` returns the literal)
om account_ledger             # 70 — structs-in-arrays v0 + expression value arms
om array_index_from_call      # 70 — structs-in-arrays v0 + expression value arms
om game_of_life               # 70 — structs-in-arrays v0 + expression value arms
om pixel_canvas               # 70 — structs-in-arrays v0 + expression value arms
om scoreboard                 # 70 — structs-in-arrays v0 + expression value arms
om self_mutation_between_calls # 70 — structs-in-arrays v0 + expression value arms
om shapes_area                # 70 — structs-in-arrays v0 + expression value arms
om stack_vm                   # 70 — structs-in-arrays v0 + expression value arms
om bit_shift                 # 64 — shifts lower to *2^k and /2^k (mul/div witnesses cover them)
om value_call_in_expr        # 70 — embedded value calls hoisted out of expressions (source-rewrite to t-vars)
om recursive_sum              # 70 — slices v0 (params, [k..], .len, machine tail-call arms)
om subslice_sum               # 70 — slices v0 (params, [k..], .len, machine tail-call arms)
om dual_accumulator_recursion # 70 — slices v0 (params, [k..], .len, machine tail-call arms)
om fletcher_checksum          # 56 — slices v0 (params, [k..], .len, machine tail-call arms)
om multi_value_calls          # 70 — slices v0 (params, [k..], .len, machine tail-call arms)
om slice_accum_probe          # 70 — struct-slice element field access `s[i].field` in a machine arm (param `&[Entry]`); the field is extracted from the (nth s i) element tuple, so both the i32-slice probe and the struct-slice probe fold to 70
om framed_payload            # 60 — bounded subslices x[a..b] via take(drop x a); fully proven
om slice_maximum             # 9 — max/min builtins as conditional binaries
om clamp_sum                 # 200 — min builtin + ℤ-mode clamped folds
om inventory_system          # 70 — 3-field record array in a nested instance (mid-field write paren fix)
om traffic_light             # 70 — case-valued nested field, zero-init normalized to the zero case
om text_greeting             # 70 — string concatenation (listcat) + concat assignment to a carrier
om status_report             # 70 — string building + comparison
om string_catalog            # 70 — String fields in NESTED structs (no declared array): `==`/`+` on deep paths self.catalog.entry1.name; needed listeq/listcat emitted for string programs, not only array programs (STR_FLAG gate)
om inventory_lookup          # 20 — string equality (structural listeq) + string-literal expression values
om text_padding              # 6  — string-carrier fields: "ALERT temp" assignment, .len arithmetic, write_line(field); dual-channel
om collatz_sequence          # 111 — hailstone steps for seed 27
om dice_roller               # 70 — self-verified
om digital_root              # 6  — digital root of 12345
om euclid_gcd                # 12 — gcd
om generic_ring_buffer       # 70 — self-verified (methods + wrap logic in range)
om insertion_sort            # 70 — self-verified (array sort)
om leap_year                 # 1
om modular_exponentiation    # 87 — 7^13 mod 100 (+ offsets)
om nested_counters           # 70 — 40 + 20 + 2*5 computed from nested data fields
om number_guess              # 70 — self-verified
om smallest_prime_factor     # 13
om tic_tac_toe               # 70 — self-verified (board array + state args)
om turn_combat               # 1
om width_mixer               # 70 — self-verified (mixed-width fields, casts in range)
om format_number             # 70 — self-verified: digit extraction via / 10 and % 10, char codes (+48), as u8 narrowing
om stdin_rot1                # 0  — EOF-driven stdin filter (read-loop to the -1 sentinel; empty input -> 0 bytes -> exit 0)
om atomics_cross             # 70 — AtomicU32 single-threaded desugar: store=field write, fetch_add=old-then-`+=`, compare_exchange=branchless swap `f + (f==e)*(n-f)`; memory orderings are no-ops

# feature tests (constructs not isolated by any committed sample):
omt cross_data               # 63 — Counter::bump/get unify self.value with Main's self.counter.value
omt bitwise_byte_mask        # 44 — compiler-profile nonnegative `x & 255` byte extraction

echo "omega meaning (Omega samples run Rust-free via omega2gamma.beta -> interp.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
