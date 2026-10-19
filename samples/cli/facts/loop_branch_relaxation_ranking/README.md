# Ranked Multi-State Countdown Loop Across Wide Conditional Branch Span

Fact: a ranked multi-state countdown loop across a wide conditional branch span establishes an exact 100u64 accumulator bound when subtracting 100u64.

Every place carries the same construct and operation: a terminating, ranked multi-state countdown loop (`loop_calc`) alternating between states `step_a` and `step_b` with measure `terminates by remaining`, spanning a wide sequence of local arithmetic computations (over 160 bytes of machine code per state) that exceeds the signed 8-bit displacement threshold (`rel8`) in x86-64 conditional branch relaxation and exercises branch offset reflow during prologue/epilogue insertion in `frame_application`. The loop terminates at `remaining == 0` with an exact accumulator postcondition `ensures result >= 100 && result <= 100`. The resulting `100u64` bound is transported across all nine canonical language places to evaluate exact subtraction `total - 100u64`, returning exit code 0.

| Place | Positive witness | Reject twin | Transport mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | A local binding receives the loop accumulator before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field carries the loop accumulator before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | An explicit machine parameter receives the loop accumulator |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A contracted forwarding machine result delivers the loop accumulator |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Reconverging match branches join on the exact loop accumulator bound |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant `const SUBTRAHEND: u64 = 100` supplies the subtrahend |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor state transition carries the loop accumulator to subtraction |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A separate evaluation machine carries the loop accumulator |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies for all four hosted targets evaluate subtraction |

## Execution

Each positive place compiles and executes with agreement between Terminal Psi and native code, exiting 0 on all four hosted platforms (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`):

```sh
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/local/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/field/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/parameter/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/call_result/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/match_join/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/const_name/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/other_state/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/other_machine/main.omg
omega run --both samples/cli/facts/loop_branch_relaxation_ranking/target_body/main.omg
```

Each place has an `invalid/` twin that breaks only the accumulator obligation by weakening the entry/loop precondition and postcondition to `99u64`, causing exact subtraction `total - 100u64` to soundly fail proof checking with `exact arithmetic in machine \`evaluate\` state \`entry\` terminal expression may overflow \`u64\`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/loop_branch_relaxation_ranking/local/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/field/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/parameter/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/call_result/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/match_join/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/const_name/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/other_state/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/other_machine/invalid/main.omg
omega --check samples/cli/facts/loop_branch_relaxation_ranking/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Ticket:** `5819b3f89f80429086463d3cec4a43e2`
- **Requirement established:** End-to-end nine-place fact preservation and native realization for ranked multi-state countdown loops spanning wide conditional branch relaxation and frame offset reflow, establishing an exact `100u64` bound for exact subtraction across all four hosted targets.
- **Obligations accounted for:**
  1. **Establishment:** Concrete entry actuals `remaining == 10` and `accumulated == 100` satisfy the loop precondition. At each cyclic transition edge, invariant arrival substitution proves `acc == 100` and `rem <= 10`, and monotonic countdown establishes the terminal postcondition `ensures result >= 100 && result <= 100` at the exit edge without circular reasoning or unproved assumptions.
  2. **Identity and lifetime:** Scalar loop operands (`rem` and `acc`) and intermediate arithmetic temporaries maintain distinct, non-aliasing value versions across all loop states (`step_a`, `step_b`, and `done`), and live storage intervals are accurately tracked across transitions.
  3. **Composition:** Across state transitions, contracted calls, match joins, record fields, machine parameters, named constants, and target-scoped bodies, the established `100u64` bound composes without loss to justify exact subtraction `total - 100u64 == 0`.
  4. **Soundness boundaries:** Reject twins demonstrate that an unproved accumulator bound (`99u64`) cannot justify exact subtraction of `100u64`, soundly rejecting during checked compilation with `exact arithmetic ... may overflow u64`.
  5. **Terminal Psi and carrier lowering:** The ranked countdown loop lowers into a strongly connected control component in Terminal Psi with typed blocks, explicit `Jump` and `Conditional` edges, well-founded termination ranking certificate (`terminates by remaining` via `Nat::Descending`), and checked invariant arrival obligations at every internal cycle edge.
  6. **Native realization, frame/layout encoding, and image emission:** The compiler executes layout-independent selected-form encoding in Stage 07, function-relative block layout in Stage 08, x86-64 conditional branch inspection in Stage 09 (retaining wide `rel32` form due to the `NearDisplacementOutsideI8` displacement exceeding 127 bytes across `step_b`'s local arithmetic block), entry prologue and epilogue insertion with branch offset reflow in `frame_application`, and executable image generation with identical exit code 0 under `omega run --both` on `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`.
- **What remains open:** Dynamic loop bounds derived from heap slices, multi-dimensional lexicographic ranking products across mutual machines, and backward branch relaxation under high register pressure spills.

Pinned corpus fixtures:
- Positive: `tests/omega/pass/facts/loop_branch_relaxation_ranking_through_<place>_exit/`
- Negative: `tests/omega/fail/facts/loop_branch_relaxation_ranking_unproved_<place>/`
