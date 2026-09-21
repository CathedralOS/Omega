# GENERAL-CYCLIC-EXECUTION-OPTIMIZER — re-verification ledger

Re-verified on Linux x86-64 at `75650d2e94` (board tip at claim time) by
Devin / w10-w10-19-general-cyclic-execution under claim ticket `a18cdb9b`.
The item re-mines the optimizer half of GENERAL-CYCLIC-EXECUTION:
`TASKS_OPTIMIZER.md:421` carries the same-named item owning the post-Terminal
stages (receiving graph, native selection, replay) starting at admitted cyclic
input; `TASKS.md:2207` owns the Psi half.

## Witnessed customer state

`tests/omega/pass/termination/ranked_callee_projected_receiver_compile`
(landed `b9c6afd4c2`) authors the item's acceptance: `Engine::run` borrows
`self.cursor` as the whole receiver of the ranked `Cursor::step`
(`terminates by remaining -> Nat::Descending`), and `main` exits 70 only when
the callee's write is observed on the projected referent. The checked-compile
+ interpreted leg **passes** at `75650d2e94`
(`cargo nextest run -p compiler --test canary_suite
domains_control_and_structures::ranked_callee_projected_receiver`).

The native leg fails. `omega run --target linux_x86_64` on the equivalent
program with the standard hosted `reaches Console` entry reports:

```
selected ProgramEntry establishment rejoins 0 Terminal attachment identities;
expected one; the machine's unit plan was omitted at an unavailable callee
(`Engine::run`)
```

Probing `checked.facts.flow.terminal_unit_effects` omissions on the authored
source gives the full chain:

- `Cursor::step` — `LocalConstruction { phase: "state graph: result
  signature" }`: `execution/unit/state_graph/returns.rs::signature` admits
  only `CheckedControlResultPlan::Unit | Structural`; a bounded scalar
  `u64 [0..=8]` result on a multi-state machine returns `None`, so no plan is
  built. (Source-read inference, not witnessed: `TerminalMachineResult::Scalar`
  exists and nothing in the verifier's `unranked_cycles::eligible` enumerates
  scalar machine results as a refusal — the witnessed stop is plan
  production, not Terminal verification.)
- `Engine::run` — `UnavailableScalarTarget { Cursor::step }` (scalar call
  target has neither a registered scalar target nor an ordinary body).
- `Main::main` — `UnavailableCallee { Engine::run }`, then the entry rejoin
  above.

A state-backedge spelling of the same machine (`true -> again(remaining - 1)`
instead of `self.step(remaining - 1)`) fails identically at the same phase:
the scalar result, not the call-cycle spelling, is the binding constraint at
production.

## Fences past the first refusal (static, verified at this revision)

- The authored `self.step(remaining - 1)` is a machine-level self-call. Even
  with a produced plan, `checked-trees-to-lowered-psi/src/unit/
  attached_unit/call_closure.rs::reject_recursive_unit_closure` ("recursive
  Unit call closure is not yet terminal") and the Terminal verifier's
  `validation/call_graph.rs` (`RecursiveCallSliceNotYetSupported`) refuse
  machine-level call cycles — runtime recursion exists at the checked/
  interpreted level only. The spec's own rule (termination.md:101) is that
  ranked tail-position recursion lowers to constant-stack iteration, i.e. a
  producer fold to block backedges; nothing emits that fold today.
- Inside cyclic bodies, `unranked_cycles::cycle_operation_eligible`'s
  `CallUnit` arm still requires `argument.path.is_empty()` — projected
  `self.<field>` receivers stay refused as call arguments within a cyclic
  machine (the persistent-receiver/whole-place rule stands).
- `terminal-psi-to-abstract-operations/src/artifact_admission/native.rs` still
  retains only whole-artifact `Vec<AcceptedControlCycle>` rosters (`:21/:43/
  :60/:81`); `into_optimization_artifact` drops the roster. No per-callee
  call/return composition exists downstream — consistent with the README's
  "ranked native admission" note.
- aot `lowering/control_flow/references.rs::call_arguments` already normalizes
  ordinary borrowed reference projections (`is_reference_projection`); the
  missing optimizer-half machinery is the cyclic-callee join: composed
  argument references across the call boundary, call/return, cleanup, callee
  measure checking, and composed resource evidence.

## Ordered next legs (ownership per the board rows, not this item)

1. Psi half (`execution/unit/state_graph` + `composed_control` + c2l
   emission): admit scalar results into `CheckedControlResultPlan` and carry
   them through composed-control emission — first blocker for both spellings.
2. Psi half: ranked runtime call-cycle admission (tail self-call fold to
   backedges, or `call_graph`/recursion-evidence support for machine members)
   — required only by the authored self-call spelling.
3. Psi half: projected `&mut self.<field>` receivers as cyclic `CallUnit`
   arguments in `unranked_cycles` with live-subloan parent-conflict checks.
4. Optimizer half (this item's scope): per-callee call/return composition in
   artifact admission and `lowering/control_flow` joins, callee measure
   checking, and composed resource evidence through native replay and
   execution on both Linux architectures.

## Live-claim state at re-verification (11:30Z)

The previously recorded fences have all drained: GENERAL-CYCLIC-EXECUTION's
claims on `execution/unit/state_graph` + `composed_control`, the
`artifact_admission` claim, and STRUCTURAL-UNIT-CALL-GRAPH-JOINS's
`lowering/control_flow` claim are expired. Currently live near the surface:

- NEW-TSVV-ELEMENT-VIEW-AOTTTO-LEG — `aot/lowering/control_flow/
  references.rs`, `lowering/structural_layout.rs`,
  `validation/reference_results.rs`, c2l
  `tests/crash_member_source/fenced_and_float_equality.rs` (exp 13:03Z).
- NEW-GCE-CYCLIC-CONTROLS-SPEC-STATUS — `wiki/spec/terminal-psi/
  verification.md` (exp 18:31Z).
- RUNTIME-DISPATCH-HELPER-LOCAL-ALIAS-ADD — `execution/unit/control/
  {statement_sequence,checked_machine}.rs` (exp 18:19Z).
- NEW-BSR-CONDITIONAL-ARM-AGREEMENT-PRODUCER — `execution/unit/
  borrowed_windows.rs` (exp 16:41Z).

`state_graph`, `composed_control`, `receiver_calls`, `call_closure.rs`,
verifier `unranked_cycles`/`call_graph`, and `artifact_admission/native.rs`
are unclaimed at this reading.

## Verdict

Still no independent slice for this row: the optimizer half's input does not
exist yet (no admitted Terminal/runtime representation of a ranked call cycle
or scalar-resulting cyclic machine reaches post-Terminal), so the remaining
work starts on the Psi half's surfaces. But the verdict is sharper than the
prior stamp: the projected-receiver acceptance is half-landed (compile +
interpret green at `b9c6afd4c2`), and the first executable leg — scalar
results in the composed state-graph route — is named with its exact stopping
phase. Sibling re-mine: RANKED-PROJECTED-RECEIVER-COMPOSITION.
