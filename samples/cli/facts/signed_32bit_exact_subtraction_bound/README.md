# Signed 32-Bit Exact Subtraction Bound

32-bit signed exact subtraction `lhs - rhs` with `80i32 <= lhs <= 120i32` and `20i32 <= rhs <= 40i32` establishes `40i32 <= diff <= 100i32` so `100i32 - diff` and `diff - 40i32` are safe across all nine places.

Every place performs the same operation: define bounded signed 32-bit inputs `lhs` (`80..120`) and `rhs` (`20..40`), evaluate exact 32-bit subtraction `diff: i32 = lhs - rhs`, transport the difference value through the place, pass `diff` to `verify_diff(diff: i32)` which requires `diff >= 40 && diff <= 100`, execute exact subtractions `head: i32 = 100 - diff` and `tail: i32 = diff - 40`, sum them to obtain `60i32`, return `sum - 60 == 0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the difference value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit signed exact subtraction evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i32 operands and their difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the difference result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the difference result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the difference result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the difference result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is subtracted from the bounded lhs |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the difference result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the difference result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 32-bit signed exact subtraction and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `lhs - rhs` with inputs `100i32` and `30i32` establishes difference `70i32` within static interval `40i32..100i32`. Passing `diff` to `verify_diff(diff)` satisfies its contract `diff >= 40 && diff <= 100`. `verify_diff` evaluates exact subtractions `head = 100 - 70 == 30i32` and `tail = 70 - 40 == 30i32`, sums them to `60i32`, and returns `sum - 60 == 0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. If subtraction or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `lhs` bound (e.g. `lhs <= 125i32` or `const RHS: i32 = 15;`), so `lhs - rhs` reaches `105i32 > 100i32`. Calling `verify_diff(diff)` fails closed at compile time with `cannot prove requires contract for call verify_diff`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_32bit_exact_subtraction_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_32bit_exact_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (each place's own exact `cannot prove requires contract for call verify_diff from <call site>: <transported bound>` fragment).

## Pinned Fixtures

The row is pinned by 18 fixtures under `tests/omega/pass/facts/signed_32bit_exact_subtraction_bound_through_<place>_exit` and
`tests/omega/fail/facts/signed_32bit_exact_subtraction_bound_through_<place>_rejected` — nine positives and their nine reject
twins, one per transport place. Each positive fixture is a byte-identical copy of its place program, binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and records native exit 0. Each reject
twin is a byte-identical copy of its place's `invalid/main.omg` and pins that place's own exact diagnostic
(`cannot prove requires contract for call verify_diff from <call site>: <transported bound>`). Golden rows live in
`tests/omega/corpus_outcomes.txt` (all 18), `tests/omega/corpus_native_macos_arm64.txt` and `tests/omega/corpus_both_macos_arm64.txt`
(the nine positives each).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 32-bit signed exact subtraction bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, 32-bit signed exact subtraction `lhs - rhs` with `80i32 <= lhs <= 120i32` and `20i32 <= rhs <= 40i32` establishes `40i32 <= diff <= 100i32` so `100i32 - diff` and `diff - 40i32` are safe across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 80 && lhs <= 120 && rhs >= 20 && rhs <= 40`) synthesize the exact difference interval `[80 - 40, 120 - 20] = [40, 100]` directly upon evaluation, falling completely within the representable signed 32-bit integer domain `[-2^31, 2^31 - 1]`. This discharges downstream precondition `diff >= 40 && diff <= 100` and exact subtractions `100 - diff` and `diff - 40` without requiring any post-hoc caller equality guard (`transition x == 70`).
  - **Identity and lifetime:** The synthesized 32-bit difference value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 32-bit signed difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_diff(diff)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 120` vs `lhs <= 125` or `RHS = 30` vs `RHS = 15`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 125` (or widened `RHS`) is specified: the difference interval expands to `[40, 105]`, which exceeds the required domain `diff >= 40 && diff <= 100`, strictly rejecting with `cannot prove requires contract for call verify_diff`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 32-bit signed subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 32-bit signed subtraction certificates (`[-2^31, 2^31 - 1]`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 32-bit signed integer layout plan (`IntegerAt`: `i32` size 4, alignment 4) validates, and target `CallPlan`/`StatePlan` assign 32-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit subtraction instructions (`sub r32, r/m32` on `x86_64`; `sub w, w, w` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `sub` on `x86_64` / `arm64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening subtraction certificates (`i32 - i32 -> i64`), vector SIMD 32-bit packed subtractions, and saturating subtraction conversions.
