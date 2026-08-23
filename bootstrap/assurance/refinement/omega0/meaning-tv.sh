#!/usr/bin/env sh
# MEANING-ROUTE TRANSLATION VALIDATION — the summit rung's Rust-free meaning gate, upgraded from shell
# comparison to KERNEL PROOF for the covered fragment.
#
# omega-meaning.sh checks each sample's meaning-route exit against its documented intent by comparing
# numbers in the shell. This gate makes that a certificate: gamma2claim.py (UNTRUSTED, the tv-encode
# precedent) abstract-executes the omega2gamma translation into an UNFOLDED kernel arithmetic term — every
# + and * in the computation is a p/m node — and proof-kernel/check.beta accepts
#       (= <meaning term> <unary exit>) (refl <unary exit>)
# only by RE-COMPUTING the sample's entire arithmetic in its own conversion. The exit is cross-checked
# three ways first (encoder = interpreter run = documented intent), and a perturbed certificate (exit+1)
# must be REJECTED, so acceptance is meaningful. Control decisions (if/match) are the encoder's, exactly
# like tv-encode's unrolled loops: a wrong decision mis-states the meaning and fails the cross-check.
# Scope: + * - / % — subtraction/div/mod via tv-encode's user-fun prelude, engaged on demand; a sample whose
# subtraction UNDERFLOWS transiently is re-encoded with ℤ DIFFERENCE-PAIR values ((pos, neg) components,
# componentwise user-fun arithmetic) and the claim P = uadd(exit, N) makes the kernel verify pos - neg =
# exit in ℤ — no negative ever materializes (the refinement pillar's move, replayed kernel-side).
# SAFETY OBLIGATIONS (lines 3+ of the encoder output): one kernel-checked claim per hazard site —
# division/mod (iszero(divisor) = 0) and ARRAY BOUNDS (omega2gamma lowers arrays to Cons spines walked by
# nth/setl whose Nil arms return silent defaults on overrun; the kernel re-computes each user-level access's
# index expression and confirms ult(idx, len) = 1, difference-pair form in zpair mode) and DOMAIN
# ERASURE (omega2gamma drops `in Saturating`/`Wrapping`; sound exactly where the domains agree with
# plain arithmetic — every subtraction site carries a kernel-checked no-underflow witness, ult(a,b)=0
# directly or d+b=a on the witnessed path; additive/multiplicative sites stay in-range by the value
# walls; ℤ-mode samples model Wrapping soundly while |values| < 2^31). omega-rs's obligations.rs
# concept, discharged by the lattice's own anchor. BOUNDARY RANGE (omega-rs boundary.rs): every value
# crossing the process boundary — the exit code and each stdout byte — carries a kernel-checked byte-range
# witness n + (255-n) = 255 (mode-uniform addition; a >255 value refuses fail-closed instead of masking).
# STRUCTURAL RESULTS (cli_mvp): a sample whose final value is a constructor tree gets a structural claim —
# the tree with computed leaves proven equal to the literal tree — and the encoder's `#render` line must
# string-equal the interpreter's printed value, pinning the claimed structure to the real run.
# DIVISION BY WITNESS: heavy arithmetic (div/mod/mul/sub over big values) no longer reduces through the
# fueled user funs (whose unfold runs blow the alpha VM's 64 MiB envelope — the old quotient wall). The
# encoder witnesses results and emits VALUE-PIN certs (= <operand term> <literal>) plus chunked literal
# addition certs proving the op's arithmetic, each an in-envelope kernel run — certifying computation,
# the multi-lemma assembly precedent applied to arithmetic. Unlocks collatz_sequence, digital_root,
# modular_exponentiation. BINARY NUMERALS: a sample whose intermediates exceed the unary wall re-encodes
# with little-endian bit-spine constructor values (BNIL/B0/B1) and carry-passing badd / shift-and-add bmul
# user funs — O(bits) unfold runs, so dice_roller's 72-million LCG states are kernel-recomputed directly.
# ALL 19 omega-meaning samples are now PROVEN. Outside: division over difference pairs (no sample needs it).
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
command -v python3 >/dev/null 2>&1 || { echo "meaning-tv: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_COMPILER_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null 2>&1 ) || { echo "meaning-tv FAIL — bc build"; exit 1; }
b() { "${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b "${OMEGA_PATH_OMEGA0}/meaning/omega2gamma.beta" "$T/omega2gamma.exe" \
  || { echo "meaning-tv FAIL — build omega2gamma.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "meaning-tv FAIL — build interp.beta"; exit 1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta  "$T/check.exe"  || { echo "meaning-tv FAIL — build check.beta"; exit 1; }

PASS=0; FAIL=0; VCTOT=0
tv() {
  src="${OMEGA_PATH_CORPUS}/$1/main.omg"
  want=$(grep -oE 'Expected exit: [0-9]+' "$src" | head -1 | grep -oE '[0-9]+')
  "$T/omega2gamma.exe" < "$src" > "$T/g" 2>/dev/null
  "$T/interp.exe" < "$T/g" > "$T/istdout" 2>/dev/null; got=$?
  case "$(head -c 6 "$T/istdout")" in '(Pair ')                   # dual-channel: exit rides the pair
    got=$(head -1 "$T/istdout" | sed 's/^(Pair \([0-9]*\) .*/\1/');; esac
  python3 "${OMEGA_PATH_OMEGA0_REFINEMENT}/gamma2claim.py" < "$T/g" > "$T/claims" 2>/dev/null \
    || { FAIL=$((FAIL+1)); echo "  FAIL $1 : encoder refused a listed sample"; return; }
  line1=$(head -1 "$T/claims"); bad=$(sed -n 2p "$T/claims")
  enc=${line1%% *}; cert=${line1#* }
  if [ "$enc" != "$got" ] || [ "$enc" != "$want" ]; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : exits disagree (encoder=$enc interp=$got documented=$want)"; return; fi
  v=$(printf '%s' "$cert" | "$T/check.exe")
  [ "$v" = accept ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : kernel rejected the meaning claim"; return; }
  v2=$(printf '%s' "$bad" | "$T/check.exe")
  [ "$v2" = reject ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : perturbed claim NOT rejected"; return; }
  nvc=0; shape=""
  while IFS= read -r vc; do
    [ -n "$vc" ] || continue
    case "$vc" in '#render '*)         # the encoder's claimed structure: must equal interp's printed value
      [ "${vc#\#render }" = "$(head -1 "$T/istdout")" ] \
        || { FAIL=$((FAIL+1)); echo "  FAIL $1 : claimed structure differs from the interpreter's"; return; }
      shape=" + structure pinned to interp stdout"; continue;; esac
    v3=$(printf '%s' "$vc" | "$T/check.exe")
    [ "$v3" = accept ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : a safety obligation was rejected"; return; }
    nvc=$((nvc+1))
  done <<VCEOF
$(tail -n +3 "$T/claims")
VCEOF
  VCTOT=$((VCTOT+nvc))
  PASS=$((PASS+1)); echo "  ok   $1 : meaning ≡ exit $enc PROVEN in the kernel (perturbed rejected; $nvc safety VCs$shape)"
}
tv bounded_counter
tv nested_counters
tv euclid_gcd
tv leap_year
tv smallest_prime_factor
tv number_guess
tv generic_ring_buffer
tv insertion_sort
tv calculator_rpn
tv tic_tac_toe
tv bank_ledger
tv bouncing_ball
tv turn_combat
tv width_mixer
tv cli_mvp
tv text_padding
tv alarm_probe
tv alarm_probe2
tv event_log
tv token_interpreter
tv nested_case_payload
tv score_tracker
tv stopwatch
tv task_runner
tv elevator
tv logger
tv alarm_scheduler
tv vending_machine
tv calculator
tv direction_command
tv parse_number
tv shape_area
tv account_ledger
tv array_index_from_call
tv game_of_life
tv pixel_canvas
tv scoreboard
tv self_mutation_between_calls
tv shapes_area
tv stack_vm
tv bit_shift
tv value_call_in_expr
tv recursive_sum
tv subslice_sum
tv dual_accumulator_recursion
tv fletcher_checksum
tv multi_value_calls
tv framed_payload
tv slice_maximum
tv clamp_sum
tv inventory_lookup
tv text_greeting
tv status_report
tv traffic_light
tv inventory_system
tv digital_root
tv collatz_sequence
tv modular_exponentiation
tv dice_roller
tv string_catalog            # String `==`/`+` on nested-struct fields: meaning PROVEN, not just run
tv format_number             # digit extraction (/ 10, % 10, char codes): meaning PROVEN
tv slice_accum_probe         # struct-slice element field access s[i].field in a tail-call: meaning PROVEN
tv atomics_cross             # single-threaded atomics desugared to field arithmetic: meaning PROVEN
echo "meaning-route TV (the kernel re-computes each covered sample's arithmetic + $VCTOT obligations: division/mul/sub witnesses + array bounds + domain-erasure + boundary-range): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] && [ "$PASS" -gt 0 ]
