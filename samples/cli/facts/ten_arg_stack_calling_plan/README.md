# Ten-Argument Stack Calling Plan

A 10-argument machine call across register and stack ABI lanes preserves the 10th stack-spilled argument bound `40u64 <= a10 <= 80u64` across all nine places so `80u64 - carried` and `carried - 40u64` are safe.

Every place performs the same operation: define bounded unsigned 64-bit input `v10` (`40..80`), invoke 10-argument machine helper `relay_ten(1, 2, 3, 4, 5, 6, 7, 8, 9, v10)` across target-specific register and stack ABI slots, transport the returned 10th argument value `carried` through the place, pass `carried` to `verify_carried(carried: u64)` which requires `carried >= 40 && carried <= 80`, execute exact subtractions `head: u64 = 80 - carried` and `tail: u64 = carried - 40`, sum them to obtain `40u64`, and verify in `main` that `res == 40` transitioning to `done()` exiting 0. Only the transport mechanism of the carried value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 10-argument register and stack call evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the 10-argument call inputs and the carried 10th argument are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the carried 10th argument is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the carried 10th argument is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the carried 10th argument is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the carried 10th argument so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the 10th argument to the 10-argument call |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the carried 10th argument to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the carried 10th argument through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body executes the 10-argument call across target ABI lanes and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

Match the fact sentence beside every place: each positive place is pinned as
`tests/omega/pass/facts/ten_arg_stack_calling_plan_through_<place>_exit` and
each reject twin as
`tests/omega/fail/facts/ten_arg_stack_calling_plan_through_<place>_rejected`,
byte-equal to the place and `invalid/` programs beside them.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/ten_arg_stack_calling_plan/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `relay_ten(1, 2, 3, 4, 5, 6, 7, 8, 9, 60)` produces `carried == 60u64` within static interval `40u64..80u64`. Passing `carried` to `verify_carried(carried)` satisfies its contract `carried >= 40 && carried <= 80`. `verify_carried` evaluates exact subtractions `head = 80 - 60 == 20u64` and `tail = 60 - 40 == 20u64`, sums them to `40u64`, and returns `40u64`. In `main`, `transition res == 40` transitions to `machine done() {}`, terminating with native exit 0. If argument transmission or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the 10th argument bound (e.g. `v10 <= 81u64` or `const V10: u64 = 81;`), so `carried` reaches `81u64 > 80u64`. Calling `verify_carried(carried)` fails closed at compile time with `cannot prove requires contract for call verify_carried`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/ten_arg_stack_calling_plan/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --filter cli/facts/nine_arg_stack_calling_plan,cli/facts/ten_arg_stack_calling_plan --jobs 2
```

Re-measured on macOS arm64 at producer revision `317bacd12b0a` (shared
generation producer): 20/20 samples (both stack-calling-plan rows' roots and
eighteen places) meet their documented oracle with 0 unscored, and 27/27
intended-invalid controls match their authored rejection — ten's nine places,
ten's nine `invalid/<place>/` mirrors, and nine's nine twins. The row author's
earlier run on a Windows host recorded this row at 10/10 samples with 18/18
controls.

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/ten_arg_stack_calling_plan_through_<place>_exit` and
`tests/omega/fail/facts/ten_arg_stack_calling_plan_through_<place>_rejected`
fixtures for the nine places, byte-equal to the place and `invalid/` programs
beside them. The positive fixtures bind `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).
Recorded at `50abcdce8fe6+wt`: nine `_exit` positions carry `checked`
(`check-target:macos_arm64`), `built ... exit:0` and
`interpreter-exit:0 interpreter:agrees` rows; nine `_rejected` twins carry the
`cannot prove requires contract for call verify_carried` diagnostic. 0 keys
lost, 0 timeout rows, 0 rejected without their expected fragment, 0 fail
fixtures that did not reject.

## Requirements and Evidence

- **Parent outcome:** `NORMALIZED-ABI-LOWERING` ([TASKS.md](../../../../TASKS.md)) — register/stack argument placement under the [calling-plan contract](../../../../wiki/spec/build/calling_plans.md).
- **Requirement established:** End-to-end nine-place fact preservation and target calling plan realization for 10-argument function calls across register and stack ABI lanes (`wiki/spec/build/calling_plans.md`). Specifically, a 10-argument machine call across register and stack ABI lanes preserves the 10th stack-spilled argument bound `40u64 <= a10 <= 80u64` across all nine places so `80u64 - carried` and `carried - 40u64` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Concrete caller actuals (`1, 2, 3, 4, 5, 6, 7, 8, 9, v10`) and bounded preconditions (`v10 >= 40 && v10 <= 80`) pass to `relay_ten(a1..a10)`. The function contract `ensures result == a10` establishes that the return value preserves `carried >= 40 && carried <= 80`, discharging downstream exact subtractions `80 - carried` and `carried - 40` without requiring post-hoc caller equality guards (`transition x == 60`).
  - **Identity and lifetime:** Positional parameter order is preserved through ABI lowering, stack argument emission, and register allocation. The 10th argument is spilled to caller-allocated stack slots across target ABIs without clobbering, displacement, or lifetime corruption.
  - **Composition:** The preserved 10th argument bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_carried(carried)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`v10 <= 80` vs `v10 <= 81` or `V10 = 60` vs `V10 = 81`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `v10 <= 81` (or `V10 = 81`) is specified: the carried interval expands to `[40, 81]`, which exceeds the required domain `carried >= 40 && carried <= 80`, strictly rejecting with `cannot prove requires contract for call verify_carried`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 10-argument machine calls and exact unsigned subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving multi-argument call dispatch and arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. Calling plan realization (`CallPlan`) partitions arguments across target-specific register and stack lanes:
    - `windows_x86_64`: args 1–4 in `RCX`, `RDX`, `R8`, `R9`; args 5–10 in caller-allocated stack slots (spill slots `[RSP + 32]` through `[RSP + 72]`).
    - `linux_x86_64`: args 1–6 in `RDI`, `RSI`, `RDX`, `RCX`, `R8`, `R9`; args 7–10 in stack slots (`[RSP]` through `[RSP + 24]`).
    - `macos_arm64` / `linux_arm64`: args 1–8 in `X0`..`X7`; args 9–10 in stack slots (`[SP]` and `[SP + 8]`).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering allocates caller argument setup frames, generates register moves and stack store instructions (`mov [rsp + disp], reg` on x86_64; `str x, [sp, disp]` on ARM64), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Calls exceeding 16 stack arguments, aggregate struct argument splitting across register/stack boundaries, indirect large struct return pointers (`sret`), and vector SIMD multi-argument calling plans.
