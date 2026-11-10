slice_0117

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0902 | `omega-rust/omega/compilation/backend/runtime/task-plans/src/stack_composition.rs:193-235` | CLOSED | Production frames always get `validation` from `task_stack_frame_validation_identity` immediately before `validate_task_stack_frame_summary` in `activation.rs:398-412`. |
| A0903 | `omega-rust/omega/compilation/backend/runtime/task-plans/src/lib.rs:319-338` | REACHABLE | `validate_wcsu_activation_plan` never inspects `unresolved_calls`, and no other `omega-rust` site rejects a non-empty set before `realize_task_activations` returns `Ok`. |
| A0904 | `omega-rust/omega/compilation/backend/register-model/src/identities.rs:184-308` | REACHABLE | `target_register_environment_identity` hashes `ieee_float_unary` and `ieee_float_directed_binary` but omits `ieee_float_directed_unary`, so `infrastructure_environment_agrees` can accept mismatched directed-unary keys. |

### A0902

1. Read `validate_task_stack_frame_summary` in full at `stack_composition.rs:193-235` (read): checks nonzero `local_bytes`, alignment, admitted same-stack alignments, and unresolved-call frame attribution; sorts/dedups `calls` and `unresolved_calls`; returns `ValidatedTaskStackFrameSummary` without reading `summary.validation` (read `stack_composition.rs:196-234`).
2. Read `task_stack_frame_validation_identity` in full at `stack_composition.rs:237-268` (read): FNV fingerprint over frame, bytes, alignment, calls, and unresolved sites (read).
3. Search: `rg '\bvalidate_task_stack_frame_summary\b' /agent/repos/omega/omega-rust` → **4 hits** (definition `stack_composition.rs:193`, re-export `lib.rs:59`, production call `activation.rs:405`, test call `stack_composition.rs:720`) (read).
4. Read production caller `TaskStorage::frame` in full at `activation.rs:338-414` (read): builds `calls` and `unresolved_calls`, then `validation = task_stack_frame_validation_identity(...)` at `activation.rs:398-404`, then passes that same tuple into `validate_task_stack_frame_summary` at `activation.rs:405-412` (read).
5. Read `realize_task_activations` at `activation.rs:64-76` (read): every frame for `compose_task_stack_demand` comes from `storage.frame` (read `activation.rs:67-68`).
6. `compose_task_stack_demand` at `stack_composition.rs:417-509` (read partial): collects `summary.validation` into `frame_validations` but does not recompute ids (read `stack_composition.rs:467-470`).

**Input reaching the accepting branch:** A `TaskStackFrameSummary` whose `validation` field is arbitrary while `local_bytes`, `alignment`, `calls`, and `unresolved_calls` satisfy the structural checks in `validate_task_stack_frame_summary`.

**Verdict: CLOSED.** Check: paired construction in `activation.rs:398-412`. Every production path to `compose_task_stack_demand` goes through `storage.frame` (`activation.rs:67-68`), which always sets `validation` from `task_stack_frame_validation_identity` using the same `identity`, `local_bytes`, `local_alignment`, `calls`, and `unresolved_calls` passed into the summary. The test at `stack_composition.rs:720-727` uses a dummy validation id `29` (read), which shows the validator does not check the field but does not affect compilation.

**Uncertainty:** None for production paths; did not read `execution_settlement.rs` beyond confirming it calls `realize_task_activations` (`execution_settlement.rs:282`).

### A0903

1. Read `validate_wcsu_activation_plan` in full at `lib.rs:319-338` (read): calls `validate_activation_plan_shape`, checks projection `has_valid_identity`, and `candidate.stack_plan == projection.stack_plan()`; does not call `projection.unresolved_calls()` or `wcsu_stack_projection().unresolved_calls()` (read).
2. Read `validate_activation_plan_shape` at `lib.rs:341-441` (read): stack nonzero size, suspension/crossing rules, canonical argument layout; no unresolved-call rule (read).
3. Search: `rg 'unresolved_calls\(\)|unresolved_calls\.is_empty' /agent/repos/omega/omega-rust` → **2 hits**, both in `stack_composition.rs` (`378`, `695`); one is a comment accessor, one hashes into projection fingerprint (read).
4. Read `realize_task_activations` at `activation.rs:69-124` (read): `compose_task_stack_demand` then `project_wcsu_stack_plan` then `validate_wcsu_activation_plan` with no intermediate unresolved-call gate (read).
5. Read Terminal ingress validation at `foundation_validation.rs:9386-9398` (read): unresolved calls only require unique state coordinates, not emptiness (read).
6. Read `compose_task_stack_demand` aggregation at `stack_composition.rs:491-507` (read): unions unresolved sites into `ComposedTaskStackDemand.unresolved_calls` with comment that non-empty means partial subgraph coverage (`stack_composition.rs:287-288`, read).

**Input reaching the accepting branch:** A `WcsuStackPlanProjection` whose `demand.unresolved_calls` is non-empty, paired with an `ActivationPlanCandidate` whose `stack_plan` matches the projection’s bytes/alignment/representation (whole resolved subgraph only).

**Verdict: REACHABLE.** A terminal task activation demand that is wire-valid and carries one or more unresolved call sites on an otherwise valid fixed-stack frame should be rejected for lacking a whole-call-graph WCSU bound, but `validate_wcsu_activation_plan` (`lib.rs:319-338`) accepts it and `realize_task_activations` returns `Ok(TaskActivationPlanSet)` (`activation.rs:145-148`).

**Uncertainty:** Did not trace every Psi lowering path that populates `TerminalTaskFrame.unresolved_calls` in `projections.rs:1838-1863`; did not read machine-emission consumers because `TaskActivationPlanSet` is not retained past `realize_task_activations` in `retained_terminal.rs:1044-1049` (read).

### A0904

1. Entry cites `identities.rs:184-308`; read `target_register_environment_identity` in full at `identities.rs:172-318` (read).
2. Confirmed optional-key loop at `identities.rs:184-250` (read) includes `ieee_float_unary` at `identities.rs:244` and `ieee_float_directed_binary` at `identities.rs:249`; **`ieee_float_directed_unary` is not hashed** (read loop; compare `constraint_catalog.rs:150` where the field exists).
3. Search: `rg '\bieee_float_directed_unary\b' /agent/repos/omega/omega-rust/omega/compilation/backend/register-model/src/identities.rs` → **0 hits** (read).
4. Read `infrastructure_environment_agrees` at `identities.rs:152-169` (read): equality uses `target_register_environment_identity(...)` recomputation, so directed-unary differences are invisible to the check.
5. Read production key source `isa-x86_64/.../machine_effects.rs:917-918` (read): `ieee_float_directed_unary` and `ieee_float_unary` are **distinct** `Some(...)` keys on x86_64.
6. Read `declared_identities.rs:283-290` (read): tests that mutating `ieee_float_directed_binary` changes identity; **no parallel test for `ieee_float_directed_unary`**.
7. Read `machine_effects.rs:155-163` and `validate_catalog_roots` at `machine_effects.rs:228-243` (read): catalog compares full `SelectedConstraintKeys` including directed unary via `terminal_selected_keys` at `machine_effects.rs:321-322`, separate from `TargetRegisterEnvironmentIdentity`.

**Input reaching the accepting branch:** Two `TargetRegisterEnvironmentConstraintKeys` values that differ only in `ieee_float_directed_unary` (with `ieee_float_unary` and other hashed fields equal), compared through `infrastructure_environment_agrees` against a single stored `TargetRegisterEnvironmentIdentity`.

**Verdict: REACHABLE.** A downstream stage that should reject register-environment evidence because directed-unary constraint selection changed would still pass `infrastructure_environment_agrees` (`identities.rs:163-169`) when the precomputed identity omits that field. Production x86_64 keys keep directed unary distinct from unary (`machine_effects.rs:917-918`), so the omission is not vacuous.

**Uncertainty:** Did not read every caller of `infrastructure_environment_agrees` beyond `spill_choice.rs:128-135` and `home_assignment.rs` reference from grep; did not determine whether any stage re-hashes directed unary outside `TargetRegisterEnvironmentIdentity`.

### What I did not read

- Full `validate_activation_plan_shape` suspension loop bodies beyond skimming `lib.rs:369-422`.
- `projections.rs` lowering logic for when unresolved calls are emitted.
- Full `home_assignment.rs` and `allocator_availability.rs` register-environment checks.
- `retained_terminal.rs` beyond `1044-1049`.
- Other area_03 digest rows.

SLICE COMPLETE
