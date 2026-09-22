# TR3-TR8 — scope verification (2026-09-20, `0f5ae41e7d`)

Board item `**TR3-TR8.**` at TASKS.md:4794. **Scope verified: six of the
seven enumerated surfaces are landed and green on origin/main; the one
remaining leg — a real selected runtime executing the ledger's transitions —
is epic-scale and its implementing surfaces are fenced this wave.**

## Landed halves (verified live at `0f5ae41e7d`)

- Whole-call-graph worst-case stack derivation + exact `StackPlan` +
  `UnresolvedCallSite` sealing: `provider-planning/src/task_plans/stack_graphs.rs`.
- Admission-time `CallTargetAssignment`s binding a sealed site to a concrete
  checked-body subtree, joining its canonical suspension crossings into the
  plan roster: `task_plans/call_target_bindings.rs` — this is the item's
  "authoritative possibly-suspending crossing roster" clause; a
  possibly-suspending call with no canonical crossing still rejects.
- Transactional arguments: `MovedTaskArguments` marshalled under the plan's
  `TaskArgumentLayout` against a plan-bound nonmoving `StackLease`,
  conserved on every start rejection (task-plans `lifecycle_ledger`,
  `provider_admission.rs`).
- Suspension/cancellation preservation: `TaskRuntimeAdmission` gates starts
  on bounded provisioning; park/resume at canonical crossings carrying each
  live place's `LiveCarryDemand`; `Cancelled` settles only against a
  recorded then observed request.
- Routed `Task<T>` establishment: `lifecycle_ledger/claim_route.rs` mints and
  resolves the exact `provider`/`activation` pair per accepted claim.

Canary evidence: all six `task_runtime` tests in `compiler::canary_suite`
pass at `0f5ae41e7d` (lifecycle claim conservation, parked-continuation
non-addressability, blocking-executor package checks, machine-selection
sidecar, custody claims, depend-edge consumer).

## The remaining leg

"A real selected runtime executing the transitions the ledger models" — real
park/resume of a native stack and safe-point observation of a checked source
(task-plans README). Nothing in `backend/runtime/{runtime-abi,
executable-installation, component-publication}` or
`native-realization/providers` carries task execution today; the ledger is a
model, not a runtime. Building it is an epic-scale leg (native stack
switching, safe-point machinery, provider dispatch) — beyond a bounded
single-session slice.

Every implementing surface it would touch is inside a live claim at
18:45Z:

- `native-realization/providers`, `terminal-psi/src`,
  `lowered-psi-to-terminal-psi/src`,
  `abstract-operations-to-target-operations/src/lowering`,
  t2c `execution/terminal_unit`, c2l `unit` → **PLACED-ACCESS-NATIVE-OPS**
  (Devin / z152, expires 2026-09-21T00:56Z)
- `task-plans`/`provider-planning` are unclaimed, but the runtime leg they
  would serve is fenced.

## Outcome

No code change made. Residual stays on TR3-TR8; dependents (ATOMIC-MEMORY-
MODEL concurrent-activation controls, WAIT-WAKE-SUBSTRATE, concurrent
activation rows) keep waiting on this execution route.

## Re-verification (2026-09-21, `0a0662ad27a`, z30)

All six landed halves re-verified green at HEAD:

- `cargo nextest run -p task-plans`: 82/82 pass (activation plans, WCSU
  stack composition and unresolved-call roster sealing, admission-time call
  target bindings, lifecycle ledger park/resume/cancellation,
  provider-admission conservation, runtime-invocation receipts).
- `cargo nextest run -p compiler --test canary_suite -E
  'test(~task_runtime)'`: 8/8 pass (lifecycle claim conservation,
  parked-continuation non-addressability, blocking-executor package checks,
  machine-selection sidecar, custody claims, depend-edge consumer).

The remaining leg is unchanged: a real selected runtime executing the
transitions the ledger models. No task-runtime vocabulary exists in
`source/library/std` (no `runtime.start`/`Task` surface), so the leg is
greenfield — language surface, selected runtime, native stack switching,
and safe-point machinery — and remains beyond a bounded slice.

Fence rotation since z142: PLACED-ACCESS-NATIVE-OPS' wholesale claim on
`native-realization/providers`, `terminal-psi/src`,
`lowered-psi-to-terminal-psi/src`, and
`abstract-operations-to-target-operations/src/lowering` has drained; those
surfaces are unfenced at 07:35Z. The shallow dependencies stay fenced:
t2c `execution/terminal_unit/{providers.rs,types/mod.rs}` under
PROVIDER-ATTACHMENT-MACHINE-PLAN (09:49Z) and c2l `src/unit` under
STRUCTURAL-UNIT-LOWERING (09:16Z). `task-plans` and `provider-planning`
remain unclaimed — the blocker is the leg's scale, not its fences.
