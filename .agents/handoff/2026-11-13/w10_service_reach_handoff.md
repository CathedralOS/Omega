# worker-10 handoff: service reach on the lowered-form route

Slot: C:\Projects\Omega\.agents\worktree\worker-10. Branch pushed as `work/psi-lowered-service-reach`
(two commits on top of a merge of origin/main 7e32e7d00). PR number and merge state: see the end of
this note.

## Done (in the branch)

Written `reaches` ceilings on checked-body machines, fail closed.

- `structure-elaboration/src/reach.rs` (`publish`, called from `finish_reading` in `lib.rs` before
  `declare::declare`): writes `TerminalModule::services` (ordered by
  `normalized_hermetic_symbol_identity` of the service's trait symbol), each machine's
  `declared_service_reach` (written ceiling, parent-closed), `published_service_ceiling` (the written
  ceiling, or for an omitted clause the least fixpoint union over `module_invocations()` callees) and
  `root_service_reach.concrete` (the entry's row). Written ceilings also go to
  `Elaboration::reach` (`ServiceReach { ceilings, names }`) because the module row cannot tell
  `reaches none` from omission. "Written" = a row in `authored_service_reach_rows_for(symbol)` or for
  its stage-04 template (`template_of`), or a non-empty `service_reach_row`.
- `read/machine.rs` `machine_shape`: no longer refuses a concrete reach row; still refuses
  installation-bound and abstract rows (`service reach bound by an installation`).
- `read/subjects.rs` `subject_declarations`: a memberless boundary trait (service identity) in the
  subject is no longer a refusal.
- `declare.rs` `declaration`: a machine with non-empty rows hashes each service identity into its
  contract commitment.
- `lowered-discharge/src/service_reach.rs` (`judge`, called from `verdict::judge` after the
  obligations): per call, callee's published row must be inside the caller's written ceiling
  (else `ExceededReach`, new list `RouteVerdict::Rejected::exceeded`, reported by
  `compiler/src/checked/checking/lowered_route.rs`) and inside the caller's published row (else
  `ForeignIdentity` table fault). Malformed rows, rows that disagree with the ceiling or root reach
  are table faults. A `Callee::Selected` invocation in a module that declares services is
  `Unsupported`.
- Fixtures: `tests/omega/pass/effects/checked_reach_ceiling_exit`,
  `tests/omega/fail/effects/checked_reach_ceiling_exceeded` (twins; only `main`'s ceiling differs).
- `OWNER_CODE_CONCERNS.md`: one entry (reach row cannot tell `reaches none` from omission).

## How far reach and boundary calls got (for the Build-program critical path)

- Reach: ONLY written ceilings on checked-body machines and direct machine-to-machine calls. Every
  contribution is a written ceiling; nothing yet adds a service that no header wrote.
- NOT built: `OperationKind::BoundaryCall` is not lowered at all. The reader still refuses any
  machine whose `supply_mode != CheckedBody` (`machine_shape`, "supply mode other than a checked
  body"), any call whose target is not a machine declaration (`Program::declaration`, "target is
  not a machine declaration"), a trait requirement with a non-empty reach row
  (`requirement_signature`, "service reach"), and any Build selection that is not a machine or data
  type (`selected_declarations`). No `BoundaryMachineDeclaration`, no `ExternalProviderConformance`
  row and no `boundary_machines` entry is written by the route.
- So a Build program (which calls `Build::*` boundary operations) is not served by this PR.

## Measured

- `route_one_comparison` (dev build, this host, before merging main):
  - pass/effects/checked_reach_ceiling_exit: current=accepted route=accepted.
  - fail/effects/checked_reach_ceiling_exceeded: current=rejected route=rejected, fragment=all;
    message `machine `main` publishes service reach `Unused` but its checked body reaches
    undeclared service `Mark`: the call of `relay` contributes `Mark + Unused``, at the call.
  - pass/calls/nonreturning_record_match_normal_exit (entry `reaches Console + ProcessExit`):
    route=outside, `Unsupported: declaration Build selection that is not a machine or a data type`
    (previously `machine service reach`). The current checker took 1,049 s on it here.
- A first draft published a private machine's derived reach instead of its written ceiling; the
  shipped checker then rejected the pass fixture (`main reaches Mark` over a private
  `relay reaches Mark + Unused`). The spec does not settle this (effects.md "not an additive
  contribution" vs "explicit published ceiling may conservatively remain wider"); the ceiling is
  taken. Owner question if the narrower reading is wanted.
- Unit tests: `mbx test -p lowered-discharge -p structure-elaboration` 215 + 60 passed after the merge.

## Exact next steps

1. Build selections (`read/subjects.rs`, `selected_declarations`): accept a selected trait /
   provider conformance. Needs the provider judged against what it provides: reuse
   `elaborate_callable_witness` (`structure-elaboration/src/lib.rs`) +
   `lowered_discharge::prove_callable_refinement` per member, and for reach the rule of
   effects.md "A selected implementation must fit every authored requirement ceiling" (compare the
   member's published row with the requirement's row in `service_reach::judge`).
2. Requirement reach: in `read/machine.rs` `requirement_signature` stop refusing a concrete
   `service_reach_row`; give the requirement machine that row as its written ceiling in
   `reach.rs::written_ceiling` (requirements are currently `None` there). Boundary-parent closure is
   already applied by `extend_closure`.
3. Boundary calls: lower a call of a boundary trait member to `OperationKind::BoundaryCall`
   (`lower/calls.rs`), declare the `BoundaryMachineDeclaration` (`fixed_service_reach`,
   `published_service_ceiling`, terminal-psi `boundary.rs:791`), and make
   `service_reach::judge` read boundary invocations too (it only walks `module_invocations()`,
   which are module machine calls). The diagnostic must then name "the identities its requirement
   declares" (effects.md).
4. Then attached data on entries (`machine_shape`, "attached data"), which is the 40-program
   refusal in the earlier table.
5. Not run anywhere: `lowered_route_*` compiler tests, `omega run --both` on the new pass fixture,
   `cargo xtask stage-coverage`, and whether Omega 00 accepts a published module with service rows
   (the compile does not publish the route's module).

## State at stop (owner stopped all work)

Draft PR https://github.com/CathedralOS/Omega/pull/2808, branch `work/psi-lowered-service-reach`,
head a6ee9f1f776c7. NOT merged, NOT ready.

The focused corpus blast is not clean and is uninvestigated:
`mbx xtask corpus --filter pass/effects/checked_reach_ceiling_exit,fail/effects/checked_reach_ceiling_exceeded`
-> the two new fixtures are right (checked; rejected expected), four neighbours moved against the
published ledger: pass/effects/checked_callback_provider_ceiling and
pass/effects/core_critical_service_catalog (checked -> rejected),
fail/effects/boundary_explicit_capability_required and fail/effects/crashes_none_rejects_trap
(rejected -> rejected unexpected). The base-commit run of those four was stopped before a result.
Suspect to check first: `pass/effects/core_critical_service_catalog` is a public machine with a
14-service `reaches` clause and an empty body, which this change now serves instead of handing
back; if the route (or the current checker under the router) rejects it, read the diagnostic.
`crashes_none_rejects_trap` uses only `reaches none`, which was served before, so it may be a row
`main` moved.

First step for whoever resumes: at origin/main run the four-fixture filter above the corpus
command printed, then the same at the PR head, capture diagnostics (the stale
`target\debug\omega.exe` in the slot cannot be used: it is from 11-11), fix, merge origin/main,
then `check-rules` with a `Proof:` line (check-rules requires it; the PR body has none yet).
Slot: clean, detached at the PR head, nothing running.
