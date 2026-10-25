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
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | dyn selector and data operand passed across state transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | dyn selector and data operand forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | dyn selector and data operand consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned fixtures

The sample folder and its fixture family do not share a name. Each place is
pinned under `tests/omega/` as
`dyn_dispatch_selector_payload_bound_through_<place>_{exit,rejected}`, following
the sibling `named_conformance_dispatch_*` and `length_*` rows:

| Place | Positive fixture | Reject twin fixture |
| --- | --- | --- |
| local | [pass/facts/dyn_dispatch_selector_payload_bound_through_local_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_local_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_local_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_local_rejected/main.omg) |
| field | [pass/facts/dyn_dispatch_selector_payload_bound_through_field_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_field_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_field_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_field_rejected/main.omg) |
| parameter | [pass/facts/dyn_dispatch_selector_payload_bound_through_parameter_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_parameter_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_parameter_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_parameter_rejected/main.omg) |
| call result | [pass/facts/dyn_dispatch_selector_payload_bound_through_call_result_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_call_result_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_call_result_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_call_result_rejected/main.omg) |
| match join | [pass/facts/dyn_dispatch_selector_payload_bound_through_match_join_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_match_join_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_match_join_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_match_join_rejected/main.omg) |
| const name | [pass/facts/dyn_dispatch_selector_payload_bound_through_const_name_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_const_name_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_const_name_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_const_name_rejected/main.omg) |
| a nested machine | [pass/facts/dyn_dispatch_selector_payload_bound_through_other_state_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_other_state_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_other_state_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_other_state_rejected/main.omg) |
| another machine | [pass/facts/dyn_dispatch_selector_payload_bound_through_other_machine_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_other_machine_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_other_machine_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_other_machine_rejected/main.omg) |
| another target body | [pass/facts/dyn_dispatch_selector_payload_bound_through_target_body_exit](../../../../tests/omega/pass/facts/dyn_dispatch_selector_payload_bound_through_target_body_exit/main.omg) | [fail/facts/dyn_dispatch_selector_payload_bound_through_target_body_rejected](../../../../tests/omega/fail/facts/dyn_dispatch_selector_payload_bound_through_target_body_rejected/main.omg) |

Each pass fixture copies its place's `main.omg` byte for byte, declares
`Expected exit: 0`, binds `ProgramEntry` for all four hosted targets, and
carries its own `expected_native_exit.txt`. Each reject twin copies its
place's `invalid/main.omg` and authors its expectation in `invalid/`-style
fragments (`exact arithmetic`, `` may overflow `u64` ``), which the corpus
runner weighs against the refusal it actually produced.

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

## Run record

Producer: `omega` built from this candidate, `--build-profile` reporting
`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`. Host:
macOS arm64. Command per place: `omega run --both <place>/main.omg`.

| Place | Positive (`--both`) | Reject twin (`--check`) |
| --- | --- | --- |
| root | `native exit 0` / `interp exit: 0` | — |
| local | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| field | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| parameter | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| call result | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| match join | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| const name | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| a nested machine | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `evaluate` local `diff` may overflow `u64` `` |
| another machine | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `forward` state `entry` terminal expression may overflow `u64` `` |
| another target body | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `compute_target` state `entry` terminal expression may overflow `u64` `` |

This row established coverage, not a compiler repair: all ten positive
projects already ran green on the base, and the gap was that no host-pinned
golden row had been recorded. No compiler source changed.

## Pinned golden records

The fact is pinned in both corpus goldens on main.

- **Check leg** (`tests/omega/corpus_outcomes.txt`): 9
  `checked ... check-target:macos_arm64` rows for the positive places and 9
  `rejected` rows for the twins, recorded at revision `cc6fcc708be` by the
  check leg filtered to this fact's 18 fixtures
  (`python3 tools/corpus_gate.py --filter <the 18 fixture paths> --record`,
  `OMEGA_CORPUS_JOBS=2`, one host-wide record slot). The reject diagnostics name
  the refusing machine and state: six places report ``exact arithmetic in machine
  `main` state `entry` local `diff` may overflow `u64` ``; a nested machine reports
  ``machine `main` state `evaluate` ``; another machine reports ``machine
  `forward` state `entry` terminal expression ``; and another target body reports
  ``machine `compute_target` state `entry` terminal expression `` once for each
  hosted target. A fresh check-leg run of the same 18 fixtures on that revision
  is clean: `0 structural moves, 0 rejected without their expected fragment,
  0 fail fixtures did not reject`.
- **Native leg** (`tests/omega/corpus_native_macos_arm64.txt`): the nine
  `built ... exit:0` rows are retained evidence stamped `@143f0b3e0428`. They
  were re-measured on revision `cc6fcc708be` by a clean `--native` run:
  `9 fixtures {'built': 9}`, 9 of 9 authored exits met, 0 stdout differences,
  no structural move (the `perf_faster` flags come from a job-count difference
  against the golden and are the gate's own noise warning). No re-record was
  needed to retain them; a later session refreshing the stamp should expect to
  change only the `@revision` token.

Both oracles were shown to discriminate on that revision, so the pinned rows
are evidence rather than a description of the environment.

- **Native oracle.** Mutating the `local` positive program's own outcome
  (`machine done() {}` to `machine done() { crash Trap; }`, which still checks) made
  the built artifact trap: the gate reported
  `expected native 0, observed built {'exit': 'signal', 'signal': '5'}`,
  1 unmet oracle, 1 structural move, exit 3, while the other eight places still
  exited 0. Restoring the program returned the native run to clean exit 0.
- **Check oracle.** Replacing one twin's authored `expected.txt` fragment with an
  unrelated sentence moved its row's `expected_satisfied` from `True` to `False`:
  the gate reported `1 rejected without their expected fragment ... it no longer
  pins its authored reason`, verdict `regressed`, exit 3. Restoring the fragment
  returned the check run to clean exit 0.

## Dispatch discrimination

The obligation's class decides where a violation is caught. For this fact the
bound is a **value/arithmetic bound** discharged by exact subtraction, so the
violation surfaces at the consuming operation; the selection itself settles at
the conformance gate.

- **Runtime, selector identity is load-bearing.** Two probes differ in exactly
  one token, the selected implementation. `ActualEvaluator` (whose member
  returns `38u64`) gives `native exit 0` / `interp exit: 0`. Swapping to
  `OtherEvaluator` (whose member returns `7u64`, so `diff` is `81`) traps both
  engines: `native signal 5` and `interp crash: Trap (host presentation
  agrees)`. The runtime dispatch reads the selected implementation's body; it
  is not a statically folded constant.
- **Static, the bound is proof information.** `88u64 - result` is provable
  only because `result <= 88u64` genuinely crosses the selector lane. Tightening
  the same program to `87u64 - result` refuses statically with ``exact arithmetic
  ... may overflow `u64` ``, so the postcondition is carried as a fact rather
  than re-derived from a known return value.
- **Static, at the conformance gate.** An implementation whose member realizes
  the requirement but weakens its postcondition (`ensures result <= 200u64`,
  returning `89u64`) is refused before any call:
  ``machine `LooseEvaluator::compute` realizes `PayloadEvaluator::compute` but
  does not guarantee its `ensures result <= 88`; callers of the requirement
  assume it``. This is the same soundness behavior the sibling
  `named_conformance_dispatch` row measured.
- **Separation holds.** Changing only the data argument (`10u64` to `9999u64`)
  while keeping the same selector and requirement leaves both engines at exit
  0, confirming the selector and data lanes are independent operands with no
  receiver attachment.
