# Unsigned 16-Bit Wrapping Subtraction Bound

16-bit unsigned wrapping subtraction `lhs -% rhs` with bounded interval `200u16 <= diff <= 450u16` establishes `200u16 <= diff <= 450u16` across all nine places so exact subtractions `450u16 - diff` and `diff - 200u16` are safe.

Every place performs the same operation: define bounded unsigned 16-bit inputs `lhs` (`300..500`) and `rhs` (`50..100`), evaluate 16-bit wrapping subtraction `(lhs as u16 in Wrapping) - (rhs as u16 in Wrapping)` producing `wrapped as u16`, transport the difference value through the place, pass `diff` to `verify_diff(value: u16)` which requires `value >= 200 && value <= 450`, execute exact subtractions `head: u16 = 450 - value` and `tail: u16 = value - 200`, sum them to obtain `250u16`, and verify in `main` that `res == 250` transitioning to `done()` exiting 0. Only the transport mechanism of the difference value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit unsigned wrapping subtraction evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded unsigned u16 operands and their wrapping difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the wrapping difference result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the wrapping difference result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the wrapping difference result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the wrapping difference result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is subtracted via wrapping subtraction from the bounded lhs |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the wrapping difference result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the wrapping difference result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 16-bit unsigned wrapping subtraction and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating wrapping subtraction with inputs `400u16` and `80u16` produces `diff == 320u16` within static interval `200u16..450u16`. Passing `diff` to `verify_diff(diff)` satisfies its contract `value >= 200 && value <= 450`. `verify_diff` evaluates exact subtractions `head = 450 - 320 == 130u16` and `tail = 320 - 200 == 120u16`, sums them to `250u16`, and returns `250u16`. In `main`, `transition res == 250` transitions to `machine done() {}`, terminating with native exit 0. If wrapping subtraction or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `lhs` bound (e.g. `lhs <= 520u16` or `const RHS: u16 = 30;`), so `diff` reaches `470u16 > 450u16`. Calling `verify_diff(diff)` fails closed at compile time with `cannot prove requires contract for call verify_diff`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_wrapping_subtraction_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_16bit_wrapping_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_diff`).

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/unsigned_16bit_wrapping_subtraction_bound_through_*_exit`
and
`tests/omega/fail/facts/unsigned_16bit_wrapping_subtraction_bound_through_*_rejected`.
Each positive fixture is a byte-identical copy of its `<place>/main.omg` plus
the place's `build.omg`; it binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and records
native exit 0 with Terminal Psi agreement. Each reject twin is a byte-identical
copy of its `<place>/invalid/main.omg` and pins the
`cannot prove requires contract for call verify_diff` diagnostic fragment (the
`lhs` bound widened from 500 to 520, or the `RHS` constant changed from 80 to
30, and nothing else).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 16-bit unsigned wrapping subtraction (`u16 in Wrapping`, `[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, 16-bit unsigned wrapping subtraction `lhs -% rhs` with bounded interval `200u16 <= diff <= 450u16` establishes `200u16 <= diff <= 450u16` across all nine places so exact subtractions `450u16 - diff` and `diff - 200u16` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 300 && lhs <= 500 && rhs >= 50 && rhs <= 100`) and 16-bit unsigned wrapping subtraction (`(lhs as u16 in Wrapping) - (rhs as u16 in Wrapping)`) establish the difference interval `[300 - 100, 500 - 50] = [200, 450]`. Because `[200, 450]` does not underflow or exceed the 16-bit unsigned integer carrier `[0, 65535]`, recasting to `u16` preserves the interval fact, discharging downstream precondition `value >= 200 && value <= 450` and exact subtractions `450 - value` and `value - 200` without requiring post-hoc caller equality guards (`transition x == 320`).
  - **Identity and lifetime:** The synthesized 16-bit wrapping difference and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 16-bit unsigned difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_diff(diff)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 500` vs `lhs <= 520` or `RHS = 80` vs `RHS = 30`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 520` (or `RHS = 30`) is specified: the difference interval expands to `[200, 470]`, which exceeds the required domain `value >= 200 && value <= 450`, strictly rejecting with `cannot prove requires contract for call verify_diff`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 16-bit wrapping subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving `WrappingIntegerSubtract` instructions and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 16-bit unsigned integer layout plan (`IntegerAt`: `u16` size 2, alignment 2) validates, and target `CallPlan`/`StatePlan` assign 16-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit subtraction instructions (`sub r16, r/m16` on `x86_64`; `sub w, w, w` with 16-bit zero truncation on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `66 2B /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening wrapping subtraction conversions (`u16 in Wrapping -> u32`), vector SIMD 16-bit packed wrapping subtractions (`psubw`), and saturating subtraction conversions (`u16 in Saturating`).
