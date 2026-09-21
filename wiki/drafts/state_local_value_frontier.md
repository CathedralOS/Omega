# STATE-LOCAL-VALUE-FRONTIER — machine_lowering leg, re-verification ledger

Board row: `TASKS.md` `**STATE-LOCAL-VALUE-FRONTIER.**` (:5598) — canonical open
item. Scoped probe on this host (linux x86-64) at `b868b9ee8f` by Zergling-126
(draft-only claim `636fee58`; the `machine_lowering` fence claim `3d28791e`
was claimed, probed, then released).

## The named `machine_lowering` leg is upstream-gated — confirmed

`checked-trees-to-lowered-psi/src/machine_lowering/machine_dispatch.rs:620`
emits `"machine has no source-independent checked scalar control plan"` when
`checked.facts.flow.terminal_scalar_graphs.for_machine(machine)` returns
`None` — the row's "missing checked scalar control plan" rejection for
`bounded_slice_selectors` and the scalar/Unit dynamic-join cases. The
rejection is correct dispatch behavior; the fix is not inside
`machine_lowering` — the row itself conditions it on "when the ordinary join
exists".

The producing surface is upstream in `typed-trees-to-checked-trees`:
`facts.rs:200` populates `flow.terminal_scalar_graphs` via
`execution/finalize_execution.rs:19`, with producers in
`execution/scalar/unit_operations.rs` (:13, :156) and
`execution/unit/state_graph/returns.rs` (:184, :194). That territory is
live-fenced this wave:

- `execution/unit/providers.rs` + neighbors — PROVIDER-ATTACHMENT-MACHINE-PLAN
  (z200, exp 09:49Z)
- `checks/multiplicity` + — BLOCKEXEC (z27, exp 10:18Z)
- `checks/termination/{progress,ranking}` — TPR6 / TERMINATION-RANKING-CHECKS
  (z27 / z159, exp 08:07Z / 10:50Z)
- `checks/contracts/exits` + c2l — PROOF-CERTIFICATION-BRIDGE (z136, exp 10:52Z)
- `flow/mutation` + — NOMINAL-FIELD-FLOW (z121, exp 08:38Z)
- `checked-trees-to-lowered-psi/src/unit` — STRUCTURAL-UNIT-LOWERING
  (Zergling-130, exp 09:16Z)

## Disposition

No implementable slice exists inside `machine_lowering` alone: removing the
`:620` rejection would only trade the documented fail-closed gate for a silent
miscompile; the graph the dispatch needs is produced behind the sibling
fences above. The row's remaining legs (caller-specific saved-argument facts,
mutable-carrier entry-value atoms, exact anonymous division across
generic/boundary calls, source-shape producer deletion) are likewise all
upstream of or beside this fence. Correct slice on this host: this ledger.
