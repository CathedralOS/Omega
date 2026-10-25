# Unsigned 64-Bit Saturating Subtraction Bound

64-bit unsigned saturating subtraction `lhs -| rhs` with bounded interval `40u64 <= diff <= 100u64` establishes `40u64 <= diff <= 100u64` across all nine places so exact subtractions `100u64 - diff` and `diff - 40u64` are safe.

Every place performs the same operation: define bounded unsigned 64-bit inputs `lhs` (`80..120`) and `rhs` (`20..40`), evaluate 64-bit saturating subtraction `(lhs as u64 in Saturating) - (rhs as u64 in Saturating)` producing `sat as u64`, transport the difference value through the place, pass `diff` to `verify_diff(value: u64)` which requires `value >= 40 && value <= 100`, execute exact subtractions `head: u64 = 100 - value` and `tail: u64 = value - 40`, sum them to obtain `60u64`, and verify in `main` that `res == 60` transitioning to `done()` exiting 0. Only the transport mechanism of the difference value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit unsigned saturating subtraction evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded unsigned u64 operands and their saturating difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the saturating difference result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the saturating difference result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the saturating difference result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the saturating difference result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is subtracted via saturating subtraction from the bounded lhs |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the saturating difference result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the saturating difference result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 64-bit unsigned saturating subtraction and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating saturating subtraction with inputs `100u64` and `30u64` produces `diff == 70u64` within static interval `40u64..100u64`. Passing `diff` to `verify_diff(diff)` satisfies its contract `value >= 40 && value <= 100`. `verify_diff` evaluates exact subtractions `head = 100 - 70 == 30u64` and `tail = 70 - 40 == 30u64`, sums them to `60u64`, and returns `60u64`. In `main`, `transition res == 60` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition res == 60` to `transition res == 61`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `lhs` bound (`lhs <= 125u64` or `const RHS: u64 = 15;`), so `diff` reaches `105u64 > 100u64`. Calling `verify_diff(diff)` fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract for call verify_diff`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_saturating_subtraction_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/unsigned_64bit_saturating_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_diff`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_saturating_subtraction_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_saturating_subtraction_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-bit unsigned saturating subtraction (`u64 in Saturating`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Specifically, 64-bit unsigned saturating subtraction `lhs -| rhs` with bounded interval `40u64 <= diff <= 100u64` establishes `40u64 <= diff <= 100u64` across all nine places so exact subtractions `100u64 - diff` and `diff - 40u64` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 80 && lhs <= 120 && rhs >= 20 && rhs <= 40`) and 64-bit unsigned saturating subtraction (`(lhs as u64 in Saturating) - (rhs as u64 in Saturating)`) establish the difference interval `[80 - 40, 120 - 20] = [40, 100]`. Because `[40, 100]` does not underflow zero or saturate, recasting to `u64` preserves the interval fact, discharging downstream precondition `value >= 40 && value <= 100` and exact subtractions `100 - value` and `value - 40` without requiring post-hoc caller equality guards (`transition x == 70`).
  - **Identity and lifetime:** The synthesized 64-bit saturating difference and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 64-bit unsigned difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_diff(diff)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 120` vs `lhs <= 125` or `RHS = 30` vs `RHS = 15`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 125` (or `RHS = 15`) is specified: the difference interval expands to `[40, 105]`, which exceeds the required domain `value >= 40 && value <= 100`, strictly rejecting with `cannot prove requires contract for call verify_diff`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 64-bit saturating subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving saturating subtraction semantics and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit unsigned integer layout plan (`IntegerAt`: `u64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit subtraction instructions with zero-clamping/conditional move (`sub r64, r/m64` with `cmovb` to zero on `x86_64`; `subs x, x, x` with `csel` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `48 2B /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening saturating subtraction conversions (`u64 in Saturating -> u128`), vector SIMD 64-bit packed saturating subtractions, and signed saturating subtraction conversions (`i64 in Saturating`).
