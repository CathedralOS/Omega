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
  t2c `execution/unit`, c2l `unit` → **PLACED-ACCESS-NATIVE-OPS**
  (Devin / z152, expires 2026-09-21T00:56Z)
- `task-plans`/`provider-planning` are unclaimed, but the runtime leg they
  would serve is fenced.

## Outcome

No code change made. Residual stays on TR3-TR8; dependents (ATOMIC-MEMORY-
MODEL concurrent-activation controls, WAIT-WAKE-SUBSTRATE, concurrent
activation rows) keep waiting on this execution route.
