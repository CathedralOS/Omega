# Signed 8-Bit Wrapping Multiplication Bound

8-bit signed wrapping multiplication `lhs *% rhs` with bounded interval `40i8 <= prod <= 96i8` establishes `40i8 <= prod <= 96i8` across all nine places so exact subtractions `96i8 - prod` and `prod - 40i8` are safe.

Every place performs the same operation: define bounded signed 8-bit inputs `lhs` (`8..12`) and `rhs` (`5..8`), evaluate 8-bit wrapping multiplication `(lhs as i8 in Wrapping) * (rhs as i8 in Wrapping)` producing `wrapped as i8`, transport the product value through the place, pass `prod` to `verify_prod(value: i8)` which requires `value >= 40 && value <= 96`, execute exact subtractions `head: i8 = 96 - value` and `tail: i8 = value - 40`, sum them to obtain `56i8`, and verify in `main` that `res == 56` transitioning to `done()` exiting 0. Only the transport mechanism of the product value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit signed wrapping multiplication evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i8 operands and their wrapping product are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the wrapping product result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the wrapping product result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the wrapping product result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the wrapping product result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is multiplied via wrapping multiplication with the bounded lhs |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the wrapping product result to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the wrapping product result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit signed wrapping multiplication and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_wrapping_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating wrapping multiplication with inputs `10i8` and `7i8` (or `10i8` and `8i8` for const_name) produces `prod == 70i8` (or `80i8`) within static interval `40i8..96i8`. Passing `prod` to `verify_prod(prod)` satisfies its contract `value >= 40 && value <= 96`. `verify_prod` evaluates exact subtractions `head = 96 - prod` and `tail = prod - 40`, sums them to `56i8`, and returns `56i8`. In `main`, `transition res == 56` transitions to `state done() {}`, terminating with native exit 0. If wrapping multiplication or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `lhs` bound (e.g. `lhs <= 13i8`), so `prod` reaches `104i8 > 96i8`. Calling `verify_prod(prod)` fails closed at compile time with `cannot prove requires contract for call verify_prod`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_wrapping_multiplication_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_wrapping_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_prod`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_wrapping_multiplication_bound_through_*` and
`tests/omega/fail/facts/signed_8bit_wrapping_multiplication_bound_unproved_*` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit signed wrapping multiplication (`i8 in Wrapping`, `[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, 8-bit signed wrapping multiplication `lhs *% rhs` with bounded interval `40i8 <= prod <= 96i8` establishes `40i8 <= prod <= 96i8` across all nine places so exact subtractions `96i8 - prod` and `prod - 40i8` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 8 && lhs <= 12 && rhs >= 5 && rhs <= 8`) and 8-bit signed wrapping multiplication (`(lhs as i8 in Wrapping) * (rhs as i8 in Wrapping)`) establish the product interval `[8 * 5, 12 * 8] = [40, 96]`. Because `[40, 96]` does not overflow or wrap around signed `i8` boundaries `[-128, 127]`, recasting to `i8` preserves the interval fact, discharging downstream precondition `value >= 40 && value <= 96` and exact subtractions `96 - value` and `value - 40` without requiring post-hoc caller equality guards (`transition x == 70`).
  - **Identity and lifetime:** The synthesized 8-bit wrapping product and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 8-bit signed product interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_prod(prod)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 12` vs `lhs <= 13`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 13` is specified: the product interval expands to `[40, 104]`, which exceeds the required domain `value >= 40 && value <= 96`, strictly rejecting with `cannot prove requires contract for call verify_prod`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 8-bit wrapping multiplication operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving wrapping multiplication semantics and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit signed integer layout plan (`IntegerAt`: `i8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit signed multiplication instructions (e.g. `imul` with truncation to 8 bits on `x86_64`; `mul` with 8-bit truncation/sign-extension on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `F6 /5` or `0F AF /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening signed wrapping multiplication conversions (`i8 in Wrapping -> i16`), vector SIMD 8-bit packed signed wrapping multiplications, and saturating signed arithmetic conversions.
