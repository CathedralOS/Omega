# slice_0105

Digest preamble read: `.agents/atlas/compiler-2026-11-10/digests/area_01.md` (Area 1: CLI, build system, compiler coordination, provider realization). Path shorthand `pp/` = `omega-rust/omega/compilation/realization/provider-planning/src/`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0808 | pp/provider_planning/provenance_replay/adapter_conformance.rs:232 | CLOSED | Empty `provider_type` skips row rejoin here, but later admission refuses selected checked adapters with empty nominal type. |
| A0809 | pp/provider_planning/provider_rows.rs:250-255 | REACHABLE | Direct-boundary capture can `continue` with no leaf and no error when machine is missing or bodied. |
| A0810 | pp/provider_planning/source_provider_selections.rs:134-137 | REACHABLE | `None` from plan selection is not always surfaced; build-time admission can `continue` silently. |
| A0811 | pp/provider_settlement.rs:263-265 | REACHABLE | `completion_roles.validate` runs only when package inputs enable semantic role discovery. |
| A0812 | pp/provider_settlement/assumed_bindings.rs:51 | REACHABLE | `(None, None)` treats calling plans as compatible when neither side has a boundary calling row. |
| A0813 | pp/provider_settlement/completion.rs:30-33 | REACHABLE | Non-`Authored` row realizations get empty external-completion leaf lists without review. |
| A0814 | pp/provider_settlement/semantic_roles.rs:51 | REACHABLE | Completion role capture applies only when schema trait name text is exactly `Console`. |
| A0815 | pp/service_declarations.rs:122-131 | REACHABLE | Absent source symbol yields `Ok(false)` and authority review treats that as no match, not an error. |

## A0808

Cited `pp/provider_planning/provenance_replay/adapter_conformance.rs:232` (read; matches digest at commit `457adf4a11f`). Read enclosing function `exact_checked_adapter` in full (`adapter_conformance.rs:107-244`, read).

At `adapter_conformance.rs:232-241` (read), `provider_identity::provider_row_matches` runs only when `!plan.provider_type.is_empty()`; when empty, the function proceeds to `validate_exact_requirement_lifetime_partition` and returns `Ok(adapter)`.

Caller search: `\bexact_checked_adapter\b` in `omega-rust` — 11 matches in 8 files (definitions/re-exports in `adapter_conformance.rs:3`, `provider_instances.rs:2`, tests/`product_boundary.rs` string guards; production call sites include `provider_instances.rs:115`, `adapter_conformance.rs:94`, `source_provider_selections.rs:22` via `selected_requirement_checked_adapter`).

Upward traces (read):

- `provider_instances.rs:102-120` prepares every selected `CheckedAdapter` row via `exact_checked_adapter` without pre-checking `provider_type`.
- `selected_requirement_checked_adapter` (`adapter_conformance.rs:88-93`, read) errors if `plan.provider_type.is_empty()` before calling `exact_checked_adapter`.
- `provider-admission/adapters.rs:59-67` (read) returns `Err` for selected plans with empty `provider_type` and `CheckedAdapter` rows before terminal admission.
- `selected-dispatch/.../operator_adapter.rs:78-82` (read) errors on empty `provider_type` for selected checked-operator plans.

Plain input: a product whose selected plan has an empty nominal provider type and a checked-adapter row; row-rejoin could fail if tested, but compilation does not reach successful terminal admission without refusal at `adapters.rs:59-67` (read).

**CLOSED** because the accepting product path that retains selected checked adapters enforces non-empty `provider_type` at admission after settlement.

## A0809

Cited `pp/provider_planning/provider_rows.rs:250-255` (read). Read `capture_direct_declarations` in full (`provider_rows.rs:213-303`, read).

At `provider_rows.rs:231-255` (read), row realization lookup yields `None` when match fails or machine has `body_is_present`, then `continue` leaves `direct[row_index]` as `None` with no diagnostic.

Caller search: `capture_direct_declarations` — 2 matches, both in `provider_rows.rs` (definition and call from `CapturedProviderRows::capture` at `:312`, read).

Upward (read): `CapturedProviderRows::attach_physical_rows` (`provider_rows.rs:321-363`) carries `None` direct slots; `finish_row_declarations` (`provider_rows.rs:420-439`) zips them into `ProviderRow { direct: None, ... }`; `produce_direct_boundary_leaves` (`selected-dispatch/intrinsic_review.rs:324-334`) only visits `candidate.direct_boundaries()` (`provider_rows.rs:77-78`, read), so omitted rows never produce leaves.

Plain input: a provider candidate with a direct-boundary-eligible row binding (`Import`/`Syscall`/intrinsic-class at `:220-227`) whose `row_realizations` entry does not resolve to a bodyless target machine at `:231-248`, or resolves to a machine with a body at `:253-254`.

**REACHABLE**: capture should reject or flag the missing direct boundary supply but instead records no leaf and no error; compile can proceed when no later stage requires that direct leaf (inferred from leaf production iterating only present `direct` inputs).

## A0810

Cited `pp/provider_planning/source_provider_selections.rs:51-55` and `:134-137` (read). Read `selected_use_plan` and `selected_requirement_plan` in full (`source_provider_selections.rs:44-138`, read).

At `:51-55` (read), invalid state-machine origin returns `None`. At `:134-137` (read), zero or multiple plan matches return `None`.

Caller search: `\bselected_requirement_plan\b` in `omega-rust` — 7 matches in 5 files; `\bselected_use_plan\b` — 10 matches in 6 files.

Production consumers (read):

- `build-settings/admission/selected_constants.rs:80-83` — on `None`, `continue` with no diagnostic.
- `selected-dispatch/.../operator_adapter.rs:172-176` — `resolve_exact_selected_plan` errors when `selected_use_plan` is `None`.
- `review-evidence/.../compiler_intrinsics.rs:189-202` — uses `selected_use_plan` for evidence joins.

Plain input: a build-time operator/requirement use with no unique selected provider plan (ambiguous or absent match at `:134-137`, or invalid machine origin at `:51-55`) during build-settings constant admission.

**REACHABLE**: admission silently skips that use (`selected_constants.rs:83`, read) instead of reporting missing selection, while other routes error—so a program could be accepted on the build-settings path when selection should fail.

## A0811

Cited `pp/provider_settlement.rs:263-265` (read). Read `settle_checked_providers` from `:160` through `:305` (read).

At `:263-265` (read), `completion_roles.validate(&completed_providers)?` runs only when `package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery)`.

Caller search: `completion_roles.validate` — 1 production call site (`provider_settlement.rs:264`).

Related (read): `program_entry/discovery.rs:30-31` returns `Ok(None)` without discovery; `compiler/checked/checking/source_checking.rs:53-58` maps missing discovery binding to `SourceCompletionAdmission::None`.

Plain input: a package compile without semantic role discovery where the selected Console/completion realization does not match source completion declarations.

**REACHABLE**: settlement skips `PreparedCompletionRoles::validate` entirely on that path, so exit-role mismatches checked only during discovery-mode compiles (e.g. package review).

## A0812

Cited `pp/provider_settlement/assumed_bindings.rs:51` (read; digest marked checked). Read `PreparedAssumedBinding::validate_calling` (`assumed_bindings.rs:38-67`, read) and `source_compatibility` (`assumed_bindings.rs:104-190`, read).

At `:43` (read), `source_compatibility?` runs first. At `:50-57` (read), `(None, None) => true` when neither required nor supplied machine appears in `boundary_calling_plan_realizations`.

Caller search: `validate_calling` on `PreparedAssumedBinding` — called from `provider_settlement.rs:277` only (read).

Plain input: an assumed binding contract whose required and supplied trait requirement symbols both lack entries in the evaluated boundary calling plan roster, while source signatures still pass `source_compatibility`.

**REACHABLE**: calling validation passes without comparing materialized signatures or entry plans, so incompatible runtime calling arrangements could survive if both sides are absent from the roster.

## A0813

Cited `pp/provider_settlement/completion.rs:30-33` (read; digest marked checked). Read `capture` in full (`completion.rs:18-57`, read).

At `:30-32` (read), when `realization.authored()` is `None`, pushes `Vec::new()` and `continue`s; external-completion review at `:40-54` runs only for `Authored` realizations with `BoundaryCompletion::External`.

Caller search: `completion::capture` — 1 call in `target_machines/prepared_providers.rs:460-465` (read), inside provider choice metadata preparation.

Upward (read): `PreparedCompletionRoles::validate` (`semantic_roles.rs:108-116`) counts external rows only when `row_external_completion_leaves` is non-empty; empty vectors do not participate.

Plain input: a selected provider row with external completion obligation realized as `BuildNative`/`Toolchain` (non-authored), not an authored checked body.

**REACHABLE**: external completion path review is skipped and recorded as an empty leaf list, so incorrect non-authored completions are not reviewed here.

## A0814

Cited `pp/provider_settlement/semantic_roles.rs:47-57` (read). Read `PreparedCompletionRole::capture` in full (`semantic_roles.rs:42-80`, read).

At `:51-56` (read), returns `None` unless `trait_name_text(&plan.schema) == "Console"` plus same-package identity guards.

Caller search: `PreparedCompletionRole::capture` — 1 call at `target_machines/prepared_providers.rs:466` (read).

Plain input: a package-owned boundary exit trait whose schema name text is not exactly `Console` (e.g. differently named process-exit trait).

**REACHABLE**: no `PreparedCompletionRole` is captured and `completion_roles.validate` (when enabled) never validates that trait’s completion role in this module.

## A0815

Cited `pp/service_declarations.rs:122-131` (read). Read `identifies` in full (`service_declarations.rs:117-139`, read).

At `:122-124` (read), missing `sources` entry returns `Ok(false)` without error.

Caller search: `.identifies(` in `omega-rust` — 1 production use: `packages/review-evidence/src/capture/authority.rs:102-111` (read).

Upward (read): on `Ok(false)`, authority capture returns `Ok(None)` at `:111`, not a diagnostic; only `Ok(true)` classifies dangerous authority.

Plain input: package authority review for a semantic binding role where the expected service declaration symbol has no matching Product source row.

**REACHABLE**: review treats the binding as unclassified rather than rejecting, so absent service evidence does not fail compilation at this gate.

## What I did not read

- Full `provider-selection` demand proving for every Import/Syscall row omitted from direct capture (A0809 acceptance end-to-end).
- Terminal pipeline stages after `settle_checked_providers` beyond `provider-admission/adapters.rs` skims (A0808).
- `wiki/spec` text for normative “should reject” requirements (all rows).

SLICE COMPLETE
