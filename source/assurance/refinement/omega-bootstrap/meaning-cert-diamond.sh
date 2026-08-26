#!/usr/bin/env sh
# MEANING-CERT CROSS-CHECK — replay the summit's certificate stream across checkers.
#
# meaning-tv.sh proves each omega-meaning sample's meaning with check.beta alone. This gate re-decides
# EVERY certificate that stream produces — meaning claims, perturbed negative controls, value pins,
# chunked witnesses, binary bit-spine arithmetic, array bounds, structural-tree claims — with the
# INDEPENDENT reference checker proof-kernel/check_ref.py AND by checker.gamma running on interp.beta (the
# table-carrying Fap translation), requiring verdict-for-verdict agreement with the built check.beta
# binary (plus the structurally expected verdict per line). One checker lying about a certificate class
# now breaks a three-checker regression cross-check instead of silently escaping the
# test corpus. This is not DDC and does not replace artifact-bound obligation reconstruction or soundness.
# The gamma leg may abstain on
# resource exhaustion — abstentions are counted and reported, never silent.
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
command -v python3 >/dev/null 2>&1 || { echo "meaning-cert diamond: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_BETA}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "meaning-cert diamond FAIL — lattice bc artifact"; exit 1; }
b() { "$BC" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b "${OMEGA_PATH_OMEGA_BOOTSTRAP}/meaning/omega2gamma.beta" "$T/omega2gamma.exe" \
  || { echo "meaning-cert diamond FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta  "$T/check.exe"  || { echo "meaning-cert diamond FAIL — build check.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "meaning-cert diamond FAIL — build interp.beta"; exit 1; }

SPECS=""
for s in bounded_counter nested_counters euclid_gcd leap_year smallest_prime_factor number_guess \
         generic_ring_buffer insertion_sort calculator_rpn tic_tac_toe bank_ledger bouncing_ball \
         turn_combat width_mixer cli_mvp text_padding alarm_probe alarm_probe2 event_log token_interpreter nested_case_payload score_tracker stopwatch task_runner elevator logger alarm_scheduler vending_machine calculator direction_command parse_number shape_area account_ledger array_index_from_call game_of_life pixel_canvas scoreboard self_mutation_between_calls shapes_area stack_vm bit_shift value_call_in_expr recursive_sum subslice_sum dual_accumulator_recursion fletcher_checksum multi_value_calls framed_payload slice_maximum clamp_sum inventory_lookup text_greeting status_report traffic_light inventory_system digital_root collatz_sequence modular_exponentiation dice_roller; do
  "$T/omega2gamma.exe" < "${OMEGA_PATH_CORPUS}/$s/main.omg" > "$T/g" 2>/dev/null
  python3 "${OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT}/gamma2claim.py" < "$T/g" > "$T/$s.claims" 2>/dev/null \
    || { echo "meaning-cert diamond FAIL — encoder refused $s"; exit 1; }
  SPECS="$SPECS $s=$T/$s.claims"
done
python3 "${OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT}/meaning_cert_diamond.py" \
  "$T/check.exe" "$T/interp.exe" "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma $SPECS
