# Dynamic dispatch selector payload bound

Dispatching a trait method through a dynamic trait carrier (`dyn` / trait conformance) whose contract establishes `result <= 88u64` preserves the return bound across the selector and data lanes, discharging exact subtraction `88u64 - result`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the dynamic trait selector and data arguments through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | dyn selector dispatches method and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | dyn selector bound to a local variable before dispatching |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | dyn selector stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | dyn selector passed as an explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | dyn selector returned across call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | dyn selector preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | dispatched result subtracted against named constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | dyn selector and data operand passed across state transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | dyn selector and data operand forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | dyn selector and data operand consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `DYNAMIC-DISPATCH` / `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place dynamic trait dispatch with explicit `dyn` selector and data lane separation, verifying that method invocation through `dyn PayloadEvaluator` statically preserves the return postcondition bound (`result <= 88u64`), discharging exact subtraction `88u64 - result` without receiver attachments.

Full 8-layer specification and lowering evidence:

- **Establishment:** Trait `PayloadEvaluator` defines inline member `machine compute(base: u64) -> u64 ensures result <= 88u64;`. Implementation `ActualEvaluator satisfies PayloadEvaluator` establishes conformance satisfaction with its verified body returning `38u64`. Coercion `ActualEvaluator as dyn PayloadEvaluator` establishes a runtime dynamic trait selector value citing `ActualEvaluator`'s closed conformance table. Dynamic call `PayloadEvaluator::compute(selector, 10u64)` dispatches through the selector and data lanes, establishing postcondition guarantee `result <= 88u64` without requiring any post-hoc caller equality guard (`transition diff == 50u64` evaluates the runtime difference `88 - 38`, rather than manufacturing the proof premise).
- **Identity and lifetime:** The dynamic selector `dyn PayloadEvaluator` carries package-scoped implementation selection identity without a data subject, `Self` carrier, receiver binding, or hidden heap allocation. The passed data argument `10u64` and returned scalar `38u64` are immutable values transported independently with distinct lifetimes. The postcondition bound `result <= 88u64` attaches strictly to the returned value across local variable binding, struct field storage and projection, explicit machine parameter passing, call returns, match joins, named constant bounds, state transitions, machine forwarding, and target-scoped bodies.
- **Composition:** The dynamic trait call result composes across all 9 standard places to statically discharge exact subtraction `88u64 - result`:
  - `local`: `dyn` selector is bound to a local variable `selector` before dispatching `compute` and subtracting.
  - `field`: `dyn` selector is stored in `Carrier.selector` record field and projected out before dispatching.
  - `parameter`: `dyn` selector is passed as an explicit machine parameter `evaluate(selector)`.
  - `call_result`: `dyn` selector is obtained from helper machine `acquire()` before dispatching `compute`.
  - `match_join`: `dyn` selectors converge across divergent match branches before dispatching `compute`.
  - `const_name`: subtraction bound is defined by compile-time named constant `const LIMIT: u64 = 88u64`.
  - `other_state`: `dyn` selector and data operand are passed as arguments to state `evaluate(sel, val)`.
  - `other_machine`: `dyn` selector and data operand are forwarded through helper machine `forward(sel, val)`.
  - `target_body`: dynamic dispatch and exact subtraction are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the trait postcondition to `ensures result <= 89u64` (and returning `89u64`), proving that exact subtraction `88u64 - result` strictly fails closed when the postcondition bound permits underflow.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow `u64``, confirming that the arithmetic proof engine soundly detects that when `result` can be 89, `88u64 - result` may underflow `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. The lowering phase strictly separates explicit `dyn` selector lanes from data argument lanes (`dyn` selector vs. data lanes), erasing mathematical proof annotations while preserving the dynamic dispatch table reference. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate `dyn` selector geometry (pointer-sized dispatch token) and 64-bit scalar data lanes, while `CallPlan`/`StatePlan` assign register and stack homes for dynamic dispatch call targets, selector operands, and scalar returns.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for indirect call dispatch through the dynamic selector table, 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic dispatch table references resolve into concrete read-only `dyn` dispatch-table relocations. Basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host platforms.

Remaining open under DYNAMIC-DISPATCH: multi-trait composite dyn selectors, dynamically sized trait object payload projections, and cross-package generic conformance specialization tables.
