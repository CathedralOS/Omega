# Signed 64-Bit Saturating Multiplication Bound

Fact: 64-bit signed saturating multiplication lhs *| rhs with bounded interval 50i64 <= prod <= 200i64 establishes 50i64 <= prod <= 200i64 across all nine places so exact subtractions 200i64 - prod and prod - 50i64 are safe.

Every place performs the same operation: define bounded signed 64-bit inputs `lhs` (`10..20`) and `rhs` (`5..10`), evaluate 64-bit saturating multiplication `(lhs as i64 in Saturating) * (rhs as i64 in Saturating)` producing `sat as i64`, transport the product value through the place, pass `prod` to `verify_prod(value: i64)` which requires `value >= 50 && value <= 200`, execute exact subtractions `head: i64 = 200 - value` and `tail: i64 = value - 50`, sum them to obtain `150i64`, and verify in `main` that `res == 150` transitioning to `done()` exiting 0. Only the transport mechanism of the product value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit signed saturating multiplication evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i64 operands and their saturating product are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the saturating product result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the saturating product result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the saturating product result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the saturating product result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is multiplied via saturating multiplication with the bounded lhs |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the saturating product result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the saturating product result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 64-bit signed saturating multiplication and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_64bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded `0` exit is strictly discriminative:
- At runtime, evaluating saturating multiplication with inputs `15i64` and `8i64` (or `15i64` and `10i64` for `const_name`) produces `prod == 120i64` (or `150i64`) within static interval `50i64..200i64`. Passing `prod` to `verify_prod(prod)` satisfies its contract `value >= 50 && value <= 200`. `verify_prod` evaluates exact subtractions `head = 200 - prod` and `tail = prod - 50`, sums them to `150i64`, and returns `150i64`. In `main`, `transition res == 150` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded value (`transition res == 150` to `transition res == 151` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` and `interp crash: Trap (host presentation agrees)`).
- Each place has an `invalid/` twin (`sample.json` + `expected.txt`) that breaks only the construct's own contract, input, or domain (never a post-hoc caller guard):
  - **Obligation class (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`):** Call contract precondition (`cannot prove requires contract for call verify_prod`) when `lhs <= 21i64` so `prod` reaches `210i64 > 200i64`, violating the required domain `value >= 50 && value <= 200` on `verify_prod`. Proof obligation checking fails closed at compile time and none reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_64bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_prod` |

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_64bit_saturating_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_prod`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_saturating_multiplication_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_saturating_multiplication_bound_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `39711ae2ed2c49098da19a7a210bd982`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-bit signed saturating multiplication (`i64 in Saturating`, `[numeric values](wiki/spec/language/numeric_values.md)` & `[integer certificates](wiki/spec/terminal-psi/integer_certificates.md)`). Specifically, 64-bit signed saturating multiplication `lhs *| rhs` with bounded interval `50i64 <= prod <= 200i64` establishes `50i64 <= prod <= 200i64` across all nine places so exact subtractions `200i64 - prod` and `prod - 50i64` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 10 && lhs <= 20 && rhs >= 5 && rhs <= 10`) and 64-bit signed saturating multiplication (`(lhs as i64 in Saturating) * (rhs as i64 in Saturating)`) establish the product interval `[10 * 5, 20 * 10] = [50, 200]`. Because `[50, 200]` does not overflow or saturate `i64` bounds `[-9223372036854775808, 9223372036854775807]`, recasting to `i64` preserves the interval fact, discharging downstream precondition `value >= 50 && value <= 200` and exact subtractions `200 - value` and `value - 50` without requiring post-hoc caller equality guards (`transition x == 120`).
  - **Identity and lifetime:** The synthesized 64-bit saturating product and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 64-bit signed product interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_prod(prod)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 20` vs `lhs <= 21`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 21` is specified: the product interval expands to `[50, 210]`, which exceeds the required domain `value >= 50 && value <= 200`, strictly rejecting under the **Contract precondition** obligation class with `cannot prove requires contract for call verify_prod`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 64-bit saturating multiplication operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving saturating multiplication semantics and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit signed integer layout plan (`IntegerAt`: `i64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit multiplication instructions with signed saturation clamping (e.g. `imul r64, r/m64` with overflow flag branch or conditional clamping on `x86_64`; `smull`/`smulh` with saturation select on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `48 0F AF /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening signed saturating multiplication conversions (`i64 in Saturating -> i128`), vector SIMD 64-bit packed signed saturating multiplications, and wrapping signed arithmetic conversions.
