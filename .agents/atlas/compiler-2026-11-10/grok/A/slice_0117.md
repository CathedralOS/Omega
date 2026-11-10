# slice_0117

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0902 | runtime/task-plans/src/stack_composition.rs:193-235 | REACHABLE | Validator never reads `summary.validation`; mismatched fingerprint is still admitted (test at :720). |
| A0903 | runtime/task-plans/src/lib.rs:319-339 | UNDECIDED | `validate_wcsu_activation_plan` skips `unresolved_calls`; no downstream rejector found; partial bound may be intentional. |
| A0904 | register-model/src/identities.rs:172-318 | REACHABLE | `ieee_float_directed_unary` is absent from the hash list while sibling float keys are included. |

## A0902

Digest claim (area_03): `validate_task_stack_frame_summary` trusts the supplied `validation` id (read: area_03 digest GROUP **task-plans**).

- Opened `validate_task_stack_frame_summary` at `omega-rust/omega/compilation/backend/runtime/task-plans/src/stack_composition.rs:193-235` (read). The function checks nonzero `local_bytes`, alignment, admitted same-stack alignments, and that unresolved sites belong to the frame; it sorts/dedups `calls` and `unresolved_calls`; it never reads `summary.validation` or calls `task_stack_frame_validation_identity` (`:237-268`, read).
- `task_stack_frame_validation_identity` hashes frame id, bytes, alignment, calls, and unresolved sites (`:237-268`, read).
- Search `rg '\bvalidate_task_stack_frame_summary\b' --glob '*.rs'` across the repository: **4** lines in **3** Rust files — definition `:193`, re-export `lib.rs:59`, production call `activation.rs:405`, unit test `stack_composition.rs:720` (read).
- Production caller `TaskStorage::frame` in `activation.rs:398-412` (read) computes `validation` via `task_stack_frame_validation_identity` then passes it into `validate_task_stack_frame_summary`; that path is consistent.
- Unit test `stack_composition.rs:720-728` (read) builds a summary with `validation: TaskStackFrameValidationId::from_normalized_identity(29)` while frame/calls use other ids; `validate_task_stack_frame_summary` returns `Ok` without checking the fingerprint.
- Downstream `compose_task_stack_demand` copies `summary.validation` into `frame_validations` and hashes it in `composition_report_fingerprint` (`:467-470`, `:597`, read) without recomputing from frame fields.

**Verdict REACHABLE (read).** Plain words: a task stack frame whose validation fingerprint does not match its local bytes, alignment, call edges, or unresolved sites, but satisfies the explicit checks above, is still validated and composed; the wrong fingerprint is carried into stack composition reports.

## A0903

Digest claim (area_03): activation-plan validation does not look at `unresolved_calls()`; who rejects a partial plan was not traced beyond the crate (read: area_03 digest).

- Opened `validate_wcsu_activation_plan` at `runtime/task-plans/src/lib.rs:319-339` (read). It calls `validate_activation_plan_shape` (`:341-441`, read remainder of that function), requires `projection.has_valid_identity()` (`:324-328`), and requires `candidate.stack_plan == projection.stack_plan()` (`:330-334`); no mention of `unresolved_calls`.
- `ComposedTaskStackDemand` stores `unresolved_calls` with comment “Non-empty means the bound covers only the resolved subgraph” (`stack_composition.rs:287-288`, read).
- `compose_task_stack_demand` (`:417-510`, read) aggregates unresolved sites from frame summaries (`:491-494`) and does not reject non-empty sets; reachability only follows `StackCallContribution::Checked` edges (`:437-448`, `:512-529`, read).
- Search `rg 'unresolved_calls\(\)' --glob '*.rs'` in `omega-rust`: **4** matches, all in `stack_composition.rs` (`:328`, `:378`, `:695`, test `:733`) (read).
- Production path `realize_task_activations` (`activation.rs:69-93`, read) runs `compose_task_stack_demand` then `validate_wcsu_activation_plan` with the projection; terminal unresolved calls are translated into frame summaries (`activation.rs:378-411`, read).
- `stack_plan_projection_report_fingerprint` hashes unresolved calls (`stack_composition.rs:695`, read), so non-empty unresolved sets change projection identity even though activation validation does not inspect them.

**Verdict UNDECIDED (read).** I could not determine from code read whether a partial call graph must be rejected at activation validation versus intentionally publishing a resolved-subgraph bound only; no `omega-rust` consumer outside `task-plans` calls `unresolved_calls()`, and reading whether `execution_settlement.rs` or Terminal Psi policy requires rejection would decide it.

## A0904

Digest claim (area_03): `ieee_float_directed_unary` is not hashed into `TargetRegisterEnvironmentIdentity` (read: area_03 digest GROUP **register-model**).

- Field present on `TargetRegisterEnvironmentConstraintKeys` at `register-model/src/constraint_catalog.rs:150` (read).
- Opened `target_register_environment_identity` at `register-model/src/identities.rs:172-318` (read). Optional float rows hashed in the first loop include `ieee_float_arithmetic`, `ieee_float_unary`, `ieee_float_conversion_control`, `ieee_float_ternary`, `float_result_trapping`, `float_result_saturating`, `ieee_float_directed_binary` (`:243-250`); `ieee_float_directed_unary` is not listed (read).
- `infrastructure_environment_agrees` at `:152-169` (read) equates environments by recomputing this identity; differing `ieee_float_directed_unary` alone would still yield equal identities.
- Separate catalog identity: `MachineEffectCatalog::selected_keys.in_identity_order()` at `selected-instructions/src/effects/catalog.rs:99-103` (read) includes `ieee_float_directed_unary` when it differs from `ieee_float_unary`; `machine_effect_catalog_identity` hashes those keys (`effects/catalog/identity.rs:101-105`, read). That is a different identity type from `TargetRegisterEnvironmentIdentity` (read).
- Search `rg 'ieee_float_directed_unary' omega-rust --glob '*.rs'`: hits in `constraint_catalog.rs`, ISA `machine_effects.rs`, `catalog.rs`, `catalog/identity.rs`, `analyses/machine_effects.rs`, tests; **none** in `register-model/src/identities.rs` (read).

**Verdict REACHABLE (read).** Plain words: two target register environments whose selected keys differ only in the directed-unary IEEE rounding row otherwise match physical model, constraints, reservations, and every other hashed key, receive the same `TargetRegisterEnvironmentIdentity` and pass `infrastructure_environment_agrees`, even when `ieee_float_directed_unary` differs from `ieee_float_unary` and would enter the machine-effect catalog identity.

## What I did not read

- `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs` unresolved-call handling (`:9387` hit only from grep).
- `omega-rust/omega/compilation/compiler/src/execution_settlement.rs` beyond the `realize_task_activations` call site.
- Full `validate_activation_plan_shape` was read through `:441`; no lines after that in the same function.

SLICE COMPLETE
