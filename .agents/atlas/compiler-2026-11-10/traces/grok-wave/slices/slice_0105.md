slice_0105

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0808 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/provenance_replay/adapter_conformance.rs:232 | UNDECIDED | Empty `provider_type` skips `provider_row_matches`; whether a mismatched adapter still fails later in instance binding was not fully traced. |
| A0809 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/provider_rows.rs:250-255 | CLOSED | Missing or bodied machine only skips direct-leaf capture; Import/Syscall rows are still realized via `external_binding_rows.rs`. |
| A0810 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/source_provider_selections.rs:51-55; 134-137 | CLOSED | `None` is intentional non-selection; product paths turn it into errors where a plan is required. |
| A0811 | omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement.rs:263-265 | CLOSED | `completion_roles.validate` runs only when `semantic_role_discovery` is true. |
| A0812 | omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/assumed_bindings.rs:51 | CLOSED | `(None, None)` passes only after `source_compatibility`; mixed `(Some, None)` still errors. |
| A0813 | omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/completion.rs:30-33 | CLOSED | Non-`Authored` realizations intentionally skip checked path review; native rows use other machinery. |
| A0814 | omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement/semantic_roles.rs:47-57 | CLOSED | Console-only capture matches discovery scope; other exit validation lives in source completion admission. |
| A0815 | omega-rust/omega/compilation/realization/provider-planning/src/service_declarations.rs:122-131 | CLOSED | `Ok(false)` on absent symbol is non-identification, not compile acceptance; sole caller treats it that way. |

### A0808

1. Read `exact_checked_adapter` in full at `adapter_conformance.rs:107-244` (read).
2. At `adapter_conformance.rs:232-241` (read), `provider_row_matches` runs only when `!plan.provider_type.is_empty()`; otherwise the row rejoin check is skipped and `validate_exact_requirement_lifetime_partition` still runs.
3. Search `rg '\bexact_checked_adapter\b' omega-rust` — 11 matches in 8 files (read); production calls: `adapter_conformance.rs:94`, `provider_instances.rs:115`; re-exports/tests elsewhere.
4. Read `CheckedAdapterDeclarations::prepare` at `provider_instances.rs:102-120` (read): every selected `CheckedAdapter` row calls `exact_checked_adapter` without the empty-`provider_type` guard used in `selected_requirement_checked_adapter` at `adapter_conformance.rs:88-92`.
5. Read upward through `provider_instances.rs:78-88` → `project_provider_instances` at `provider-selection/src/binding/provider_instances.rs:36-194` (partial): binding aggregates diagnostics but I did not read `inherited_instances` in full.
6. Input to accepting branch (read): a `ProviderPlan` with empty `provider_type` (e.g. `plan_derivation.rs:111` uses `unwrap_or_default()`), `CheckedAdapter` row, and a machine that passes identity/package checks in `exact_checked_adapter`.

**Verdict:** UNDECIDED — the skip is real (read), but I did not prove that `inherited_instances` / `project_provider_instances` would still accept a machine that `provider_row_matches` would reject at `provider_identity.rs:252-259` (read).

**Unsure:** End-to-end fate of wrong attached-data on empty-`provider_type` CheckedAdapter plans after `provider_instances.rs:115`.

### A0809

1. Read `capture_direct_declarations` in full at `provider_rows.rs:213-303` (read).
2. At `provider_rows.rs:231-255` (read), `None` from realization lookup or `machine.body_is_present` causes `continue` with no diagnostic; `direct[row_index]` stays `None`.
3. Read file header `provider_rows.rs:1-6` (read): direct identity errors are deferred to other stages.
4. Read `extract_native_external_binding_rows` consumer path: `provider_settlement.rs:230-239` (read); `external_binding_rows.rs:103-123` (read) builds Import/Syscall rows without the direct-capture machine.
5. Read `produce_direct_boundary_leaves` at `intrinsic_review.rs:324-334` (read): only iterates `candidate.direct_boundaries()`, so absent capture means no direct leaf, not a silent substitute.

**Verdict:** CLOSED — for Import/Syscall bindings, normalization goes through `external_binding_rows.rs:119-123` (read); the `continue` at `provider_rows.rs:250-255` only skips optional direct-declaration capture.

**Input:** derived plan row with `Import`/`Syscall`/etc. binding where no bodyless matching machine exists or machine has a body.

### A0810

1. Read `selected_use_plan` and `selected_requirement_plan` in full at `source_provider_selections.rs:44-138` (read).
2. At `source_provider_selections.rs:51-55` (read), invalid state-machine origin returns `None`.
3. At `source_provider_selections.rs:134-137` (read), zero or multiple plan matches return `None`.
4. Search `rg '\bselected_requirement_plan\b' omega-rust` — 7 matches in 5 files (read).
5. Read callers: `operator_adapter.rs:172-176` (read) errors via `resolve_exact_selected_plan`; `selected_constants.rs:80-83` (read) `continue`s; `compiler_intrinsics.rs:189-227` (read) errors when a provider is required; `application_realization.rs:197-203` (read) returns `Ok(None)` only on the canonical-empty path (read partial context).

**Verdict:** CLOSED — product dispatch and review evidence turn missing selection into errors where needed (`operator_adapter.rs:172-176`, `compiler_intrinsics.rs:225-227`).

**Unsure:** Whether every build-settings `continue` at `selected_constants.rs:82` has a later mandatory check for every requirement kind (not read in full).

### A0811

1. Read `settle_checked_providers` at `provider_settlement.rs:242-265` (read).
2. At `provider_settlement.rs:263-265` (read), `PreparedCompletionRoles::validate` runs only if `package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery)`.
3. Read `PackageCompilationInputs` default at `package-compilation/src/package_compilation.rs:116` (read): `semantic_role_discovery: false`.
4. Read parallel gate at `program_entry/discovery.rs:30-31` (read) and `source_checking.rs:53-58` (read): discovery vs accepted vs none for source completion.

**Verdict:** CLOSED — the accepting branch (skipping role validation) is only taken when `semantic_role_discovery` is false, which is the ordinary compile default (read).

### A0812

1. Read `PreparedAssumedBinding::validate_calling` in full at `assumed_bindings.rs:38-67` (read).
2. At `assumed_bindings.rs:50-57` (read), `(None, None) => true`; asymmetric cases `=> false`.
3. At `assumed_bindings.rs:43` (read), `source_compatibility?` runs first (`assumed_bindings.rs:104-190`, read in full for `source_compatibility`).
4. Read caller `provider_settlement.rs:276-278` (read) and competing-supply checks `provider_settlement.rs:279-303` (read).

**Verdict:** CLOSED — `(None, None)` accepts only after signature/lifetime compatibility (read) and only when neither side has a boundary calling row; partial presence still errors at `assumed_bindings.rs:56-64` (read).

**Input:** assumed-binding contract whose required and supplied requirement machines have no matching `BoundaryCallingPlanRealization` rows.

### A0813

1. Read `completion::capture` through the row loop at `completion.rs:18-57` (read).
2. At `completion.rs:30-33` (read), `!realization.authored()` pushes empty leaf list without error.
3. Read `ProviderRowRealization::authored` at `provenance_replay.rs:36-42` (read): only `Authored` is reviewed here.
4. Read `PreparedCompletionRole::validate` at `semantic_roles.rs:108-117` (read): external rows count only nonempty `row_external_completion_leaves`.

**Verdict:** CLOSED — checked completion-path review applies only to `Authored` realizations (read); `BuildNative`/`Toolchain` are handled outside this authored-body walker (read comment at `completion.rs:1-6`).

**Unsure:** Whether an external-completion requirement can be *selected* with only `Toolchain` realization without other refusal (selection rules not read).

### A0814

1. Read `PreparedCompletionRole::capture` in full at `semantic_roles.rs:41-80` (read).
2. At `semantic_roles.rs:47-57` (read), returns `None` unless trait name text is exactly `"Console"` with package alignment.
3. Read `semantic_roles.rs:98-151` `validate` (read): only processes `Some(role)`.
4. Read `declarations_core.rs:1559-1606` (read): discovery nomination also requires display path `"Console"`.

**Verdict:** CLOSED — this site validates Console completion roles during discovery settlement only; differently named exit traits are out of scope here and use `SourceCompletionAdmission` paths (read `source_checking.rs:47-58`).

### A0815

1. Read `identifies` in full at `service_declarations.rs:117-139` (read).
2. At `service_declarations.rs:122-124` (read), absent symbol returns `Ok(false)`.
3. Search `rg '\.identifies\(' omega-rust` — 1 match (read): `authority.rs:103`.
4. Read caller at `authority.rs:94-111` (read): only `true` selects a dangerous authority class; `false` falls through to `Ok(None)`.

**Verdict:** CLOSED — `Ok(false)` means “does not identify this declaration,” not program acceptance (read).

## What I did not read

- Full `inherited_instances` and checked-facts projection in `provider_instances.rs` (beyond ~line 245).
- Full `validate_selected_operators` / `validate_selected_provider_bodies` after `selected_constants.rs:82` `continue`.
- Provider-selection rules for pairing `BoundaryCompletion::External` with `Toolchain`/`BuildNative` realizations.
- Test-only callers of `exact_checked_adapter` beyond skimming hit lists.
- Remainder of `adapter_conformance.rs` after line 299 (`validate_exact_requirement_lifetime_partition` tail).

SLICE COMPLETE
