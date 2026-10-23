# Fact: Live Across Twenty-Seven Calls Preservation Bound

`samples/cli/facts/live_across_twentyseven_calls_preservation_bound` demonstrates that a
value live across twenty-seven sequential machine calls preserves its static bound
`40u64 <= saved <= 80u64` across all nine canonical specification places, so exact
subtractions `80u64 - res` and `res - 40u64` remain statically provable and execute
without underflow on all four hosted targets.

```
// Fact: a value live across twenty-seven sequential machine calls preserves its static bound 40u64 <= saved <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.
```

## Places

| Place | Positive | Reject twin | Summary |
|---|---|---|---|
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the preserved value `saved` is preserved across twenty-seven sequential calls and used locally |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the preserved value `saved` is packed into and read from a struct field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the preserved value `saved` is forwarded as a parameter to a helper machine |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the preserved value `saved` is returned from a callee invocation preserving bounds |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the preserved value saved so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a bounded constant `SAVED` is preserved and used across twenty-seven sequential calls |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the preserved value saved to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the preserved value saved through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body preserves the bounded value across twenty-seven sequential calls |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/local/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/field/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/parameter/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/call_result/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/match_join/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/const_name/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/other_state/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/other_machine/main.omg
omega run --both samples/cli/facts/live_across_twentyseven_calls_preservation_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `compute_*(60u64)` across twenty-seven sequential helper calls preserves `saved == 60u64` within interval `40u64..80u64`. Performing exact subtractions `head: u64 = 80 - 60 == 20u64` and `underflow: u64 = 60 - 40 == 20u64` yields `head + underflow == 40u64`. In `main`, `transition outcome == 0` transitions to `state done() {}`, terminating with native exit 0. If preservation fails or corrupts the live value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the bound contract to `saved >= 39 && saved <= 80` (or `SAVED = 39`), so `res - 40` may underflow. Exact subtraction `let underflow: u64 = res - 40;` fails closed at compile time with `exact arithmetic in machine ... local underflow may overflow u64`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/live_across_twentyseven_calls_preservation_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/live_across_twentyseven_calls_preservation_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`underflow`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/live_across_twentyseven_calls_preservation_bound_through_<place>_exit` (nine positives: `main.omg`, `build.omg`, `expected_native_exit.txt`) and `tests/omega/fail/facts/live_across_twentyseven_calls_preservation_bound_through_<place>_rejected` (nine reject twins: `main.omg`, `build.omg`, `expected.txt`). Every fixture is a byte-identical copy of its place program, so the sample and the fixture cannot drift apart. The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Recorded from the shared opt-level 1 producer on macOS arm64: `corpus_outcomes.txt` gains nine `checked` rows carrying `check-target:macos_arm64` and nine `rejected ... expected` rows with their own diagnostic line, `corpus_native_macos_arm64.txt` gains nine `built ... exit:0` rows, and `corpus_both_macos_arm64.txt` gains nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for live-across-call values across twenty-seven sequential calls in callee-saved registers and stack spill homes (`[storage](wiki/spec/resources/storage.md)`, `[activation storage](wiki/spec/resources/activation_storage.md)`, `[calling plans](wiki/spec/build/calling_plans.md)`). Specifically, a value live across twenty-seven sequential machine calls preserves its static bound `40u64 <= saved <= 80u64` across all nine places so exact subtractions `80u64 - res` and `res - 40u64` are safe without underflow.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Static requires precondition `requires saved >= 40 && saved <= 80` establishes that `saved` is bounded in `[40, 80]`. The value remains live across twenty-seven sequential machine calls (`step1` through `step27`) without losing its static interval.
  - **Identity and lifetime:** The preserved value and its static interval bounds survive across call clobber boundaries in callee-saved registers or dedicated stack activation spill slots, and across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans across twenty-seven sequential call sites.
  - **Composition:** The static bound `40u64 <= saved <= 80u64` composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtractions (`80 - res` and `res - 40`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`saved >= 40` vs `saved >= 39` or `SAVED = 60` vs `SAVED = 39`), leaving the exact subtraction obligation unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `saved >= 39` (or `SAVED = 39`) is specified: the lower bound falls below 40, which causes exact subtraction `res - 40` to fail compile-time checking with `exact arithmetic in machine ... local underflow may overflow u64`. No unproved underflowing subtraction reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Sequential machine calls and live value preservation lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving callee-saved preservation and disjoint spill home demands. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit integer layout plan (`u64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign register and stack spill placements for values live across calls per ABI rules.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions, assigns callee-saved register homes (such as `rbx`/`r12-r15` on `x86_64`; `x19-x28` on `arm64`) or spill slots in the activation frame, and emits prologues and epilogues restoring callee-saved registers according to the `StatePlan`. Twenty-seven sequential calls exert sustained pressure on callee-saved allocation, spill slot allocation, and worst-case stack usage (WCSU) activation frame sizing.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Values live across 32 or more sequential calls with extreme register pressure forcing multi-slot stack frame spill reallocation, live-across-call floating point scalar preservation, and live-across-call vector register preservation.
