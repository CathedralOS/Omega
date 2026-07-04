#!/usr/bin/env sh
# MEANING-CERT DIAMOND — D5 (diversity at every seam) for the summit's certificate stream.
#
# meaning-tv.sh proves each omega-meaning sample's meaning with check.beta alone. This gate re-decides
# EVERY certificate that stream produces — meaning claims, perturbed negative controls, value pins,
# chunked witnesses, binary bit-spine arithmetic, array bounds, structural-tree claims — with the
# INDEPENDENT reference checker delta/check_ref.py AND by checker.gamma running on interp.beta (the
# table-carrying Fap translation), requiring verdict-for-verdict agreement with the built check.beta
# binary (plus the structurally expected verdict per line). One checker lying about a certificate class
# now breaks a THREE-checker diamond instead of silently anchoring trust. The gamma leg may abstain on
# resource exhaustion — abstentions are counted and reported, never silent.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "meaning-cert diamond: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "meaning-cert diamond FAIL — bc build"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b omega2gamma.beta     "$T/e2g.exe"    || { echo "meaning-cert diamond FAIL — build omega2gamma.beta"; exit 1; }
b ../delta/check.beta  "$T/check.exe"  || { echo "meaning-cert diamond FAIL — build check.beta"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "meaning-cert diamond FAIL — build interp.beta"; exit 1; }

SPECS=""
for s in bounded_counter nested_counters euclid_gcd leap_year smallest_prime_factor number_guess \
         generic_ring_buffer insertion_sort calculator_rpn tic_tac_toe bank_ledger bouncing_ball \
         turn_combat width_mixer cli_mvp text_padding alarm_probe alarm_probe2 event_log token_interpreter nested_case_payload score_tracker stopwatch task_runner elevator logger alarm_scheduler vending_machine calculator direction_command parse_number shape_area account_ledger array_index_from_call game_of_life pixel_canvas scoreboard self_mutation_between_calls shapes_area stack_vm bit_shift value_call_in_expr recursive_sum subslice_sum dual_accumulator_recursion fletcher_checksum multi_value_calls digital_root collatz_sequence modular_exponentiation dice_roller; do
  "$T/e2g.exe" < "../../samples/$s/main.omg" > "$T/g" 2>/dev/null
  python3 gamma2claim.py < "$T/g" > "$T/$s.claims" 2>/dev/null \
    || { echo "meaning-cert diamond FAIL — encoder refused $s"; exit 1; }
  SPECS="$SPECS $s=$T/$s.claims"
done
python3 meaning_cert_diamond.py "$T/check.exe" "$T/interp.exe" ../gamma/checker.gamma $SPECS
