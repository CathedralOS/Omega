slice_0106

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0816 | `provider-planning/src/service_schema/mod.rs:44` | CLOSED | Only `BoundaryTrait` schemas get ABI attachments; other kinds use separate calling-policy paths and native schema validation. |
| A0817 | `provider-selection/src/binding.rs:159` | UNDECIDED | Zero matching granted plans is explicitly allowed; did not trace whether Psi `AdmittedReceipt` issuers always imply a grant-backed method. |
| A0818 | `provider-selection/src/binding/provider_instances.rs:71,81,99` | CLOSED | Skips are for non-instance binding kinds, empty optional candidates, or non-matching applications; `required_instances` enforces closed requirements. |
| A0819 | `provider-selection/src/bodies.rs:184-187` | CLOSED | Missing numeric provider returns retained only after `reconcile_numeric_meaning`; Terminal boundary admission rejects `None` execution for intrinsics. |
| A0820 | `provider-selection/src/bodies/callback_roots.rs:174-176` | REACHABLE | External registration with no selected row is treated as non-foreign and adds no root despite checked entry and a calling plan. |
| A0821 | `provider-selection/src/bodies/target_bodies.rs:30-32,76-80,99` | CLOSED | Skips and empty frontier are documented negative-evidence behavior; inert reach is checked when entry is present via `execution_settlement.rs:357`. |
| A0822 | `provider-selection/src/publication.rs:494-497,503-506` | CLOSED | Skips apply only to machines and operations outside the computed reachability set for demand growth. |
| A0823 | `selected-dispatch/src/compiler_intrinsic.rs:136-138` | CLOSED | `Unsupported` is retained as `None` and package review re-derives via `intrinsics.rs:264-273`. |

### A0816

1. Read `attach_requirement_calling` and `attach_calling` in full (`service_schema/mod.rs:18-67`, read).
2. Search `rg '\battach_calling\b' omega-rust --glob '**/*.rs'`: 10 files, 11 mention lines (read service_declarations.rs:74, prepared_providers.rs:129,278-286).
3. Read `ServiceDeclarations::prepare` (`service_declarations.rs:61-98`, read) and `PreparedProductProviders::attach_calling` (`prepared_providers.rs:255-286`, read); did not read every provenance_replay caller.
4. Input on the accepting branch: `PreparedServiceSurface` whose `schema` is not `ProviderSchemaDeclaration::BoundaryTrait(_)` (e.g. operator, requirement, sealed accessor from read grep hits in `plan_derivation.rs`).
5. **Verdict: CLOSED.** Reason: Line 44 gates attachment to `BoundaryTrait`; non-trait schemas return `ServiceSchema` with empty `calling_applications` by design. `provider_settlement` / `external_binding_rows` supply calling policy elsewhere (`provider_settlement.rs:316-339`, `external_binding_rows.rs:165-176`, read). Native candidates still hit `validate_candidate_against_schema` (`prepared_providers.rs:278-286`, read).
6. **Unsure:** None for this entry.

### A0817

1. Read `validate_qualification_grants` in full (`binding.rs:87-170`, read).
2. Search `rg '\bvalidate_qualification_grants\b' omega-rust`: 3 matches in `binding.rs` + `lib.rs` reexport; production caller is `bind_selected_provider_plan_facts` (`binding.rs:76-80`, read), then `provider_binding.rs:155-173` (read).
3. Read `bind_selected_provider_plan_facts` through `boundary_admission.admit` (`binding.rs:58-85`, `boundary.rs:158-208`, read); did not read full `resolve_authored_selected_provider_grants`.
4. Input on the accepting branch: an admitted `BoundaryQualificationIssuer` from `admitted_qualification_issuers()` (`facts/src/lib.rs:1280-1298`, read) whose `requirement_identity` matches zero entries in `granted_plans` filtered by schema methods (`binding.rs:117-134`, read).
5. **Verdict: UNDECIDED.** Reason: `[] | [_] => {}` at `binding.rs:158-159` (read) deliberately allows zero matches; comments at `binding.rs:13-17` discuss ambiguity and owner fallback, not mandating ≥1 match.
6. **Unsure:** Whether Psi checking guarantees every `AdmittedReceipt` issuer has at least one granted selected plan whose schema methods cover `requirement_identity` before this runs.

### A0818

1. Read `project_provider_instances` in full (`provider_instances.rs:36-194`, read).
2. Search `rg '\bproject_provider_instances\b' omega-rust`: 4 files; production path `provider_instances.rs:75` via `PreparedProviderInstances::project_instances` (`provider_instances.rs:61-76`, read).
3. Read `bind_selected_provider_plan_facts` enrichment path only indirectly; read `required_instances` loop (`provider_instances.rs:172-185`, read).
4. Input on accepting branches: (a) bindings other than CheckedAdapter/CompilerIntrinsic/Syscall (`:64-71`); (b) `application.candidates.is_empty()` (`:81-82`); (c) zero candidates after filters (`:98-99` continue).
5. **Verdict: CLOSED.** Reason: Non-covered bindings are not instance-attachment kinds here (read). Empty or skipped candidates still fail closed for `template.required_instances` when `matching != 1` (`:180-184`, read). Errors from malformed applications accumulate and return `Err` (`:187-189`, read).
6. **Unsure:** Whether optional inherited applications with empty candidates can leave a gap never checked elsewhere (did not read full instance declaration preparation).

### A0819

1. Read `selected_execution_for_identity` and `selected_external_execution` in full (`bodies.rs:144-197`, read).
2. Search `rg 'selected_external_execution|selected_execution_for_identity' omega-rust`: 7 files; read callers `boundary.rs:98-111` and `boundary.rs:284` (partial), `retained.rs:228-250` (read).
3. Read `reconcile_numeric_meaning` (`effects/src/external_providers.rs:153-171`, read).
4. Input: `selected_numeric_provider` returns `None` (`bodies.rs:184-186`, read); `retained` may be `None` or non-numeric `Some`.
5. **Verdict: CLOSED.** Reason: `reconcile_numeric_meaning(None, retained)` errors when `retained` is numeric (`external_providers.rs:168-169`, read). Terminal boundary path requires `execution.ok_or_else(...)` (`boundary.rs:109-111`, read). `retained.rs` treats `None` identity as `NoClosedCatalogIdentity` (`retained.rs:254-256`, read), not silent success with wrong meaning.
6. **Unsure:** None.

### A0820

1. Read `select_callback_declarations` and `registration_is_foreign` in full (`callback_roots.rs:100-186`, read).
2. Search `rg '\bselect_callback_declarations\b' omega-rust`: 6 files; production `provider_binding.rs:194-197` (read); tests in `callback_body_controls.rs` (not read in full).
3. Read `CallbackProviderDeclarations::selected_row` (`callback_roots.rs:81-97`, read).
4. Input: `CallbackRegistrationDeclaration::External(identity)` with `providers.selected_row(identity)` = `None` → `registration_is_foreign` returns `Ok(false)` (`:174-176`, read).
5. **Verdict: REACHABLE.** Reason: A callback use with `has_checked_entry`, exactly one calling-plan match, and external registration identity that does not resolve to a selected row is not foreign (`:155-157` never runs), adds no root, and can return `Ok(roots)` if no other diagnostics (`:159-163`, read)—a program that should fail foreign-callback placement could be accepted.
6. **Unsure:** Whether Terminal always supplies a selected row for every external registration identity before this runs (did not read full callback preparation in Psi).

### A0821

1. Read `DirectTargetBodyAbsences::derive` and `reject_reached_inert_calls` in full (`target_bodies.rs:19-127`, read).
2. Search `reject_reached_inert_calls`: `target_machines.rs:125`, `execution_settlement.rs:357` (read); `DirectTargetBodyAbsences::derive` in `plan_derivation.rs:186-204` (read).
3. Read `execution_settlement.rs:357-364` (read); did not read full reachability inside `applications.reachability`.
4. Input: calls with `call.machine == None` (`:30-32`); `entry` `None` → empty `frontier` (`:76-80`); `explicit_body` returns `None` → refusal or continue (`:99-110`).
5. **Verdict: CLOSED.** Reason: File header (read lines 1-8) states absence is negative evidence. Empty entry yields `Ok(())` only when no refusals are collected (`:119-120`, read). Product settlement still runs `reject_reached_inert_sibling_calls` when entry is known (`execution_settlement.rs:357-364`, read). Absent bodies filter plans in derivation (`plan_derivation.rs:193-203`, read).
6. **Unsure:** Behavior when `entry` is `None` but unreachable inert direct calls exist outside the walked graph (inferred from empty frontier, not fully traced through `reachability`).

### A0822

1. Read `extend_application_demand` (`publication.rs:412-539`, read), focused `493-518`.
2. Search `rg '\bextend_application_demand\b'`: only `publication.rs` (read `select` loop `:318-409`).
3. Read `PreparedTargetChoices::select` fixed-point loop (`publication.rs:318-409`, read); did not read `applications.reachability` implementation.
4. Input: reachable machine body owner not in `scope.callable_declaration` for Machine (`:494-497` continue); terminal operation with no matching `boundary_application_demands` entry (`:503-506` continue).
5. **Verdict: CLOSED.** Reason: Skips only affect demand marking for machines/operations not in the reachable set built from roots and edges (`:460-472`, read); non-reachable providers are intentionally not demanded (inferred non-boundary / out-of-reach from read comments at `publication.rs` context).
6. **Unsure:** Exact correspondence between `reachable.machines` and programs that should still demand a provider (did not read reachability implementation).

### A0823

1. Read `settle_declaration` (`compiler_intrinsic.rs:127-140`, read) and `SelectedCompilerIntrinsicExecutionIdentity` enum (`:22-24`, read).
2. Search `rg '\bsettle_declaration\b' omega-rust`: `compiler_intrinsic.rs` + `intrinsic_review.rs:246-252` (read).
3. Read `retain_selected_compiler_intrinsic_review_identities` match arm (`intrinsic_review.rs:254-270`, `:315-321`, read) and `reconcile_compiler_intrinsic_execution` (`packages/review-evidence/.../intrinsics.rs:264-273`, read).
4. Input: `declaration.settle(...)?` returns `None` → `Unsupported` wrapped in `Ok` (`compiler_intrinsic.rs:135-138`, read).
5. **Verdict: CLOSED.** Reason: `Unsupported` becomes `rows.push(None)` (`intrinsic_review.rs:262-264`, read); package review maps `Unsupported` + `retained None` to `Ok(None)` and errors if retained is spoofed (`intrinsics.rs:264-269`, read); missing closed identity errors at `:271-273` (read).
6. **Unsure:** None.

### What I did not read

- Full bodies of `plan_derivation.rs`, `applications.reachability`, `resolve_authored_selected_provider_grants`, and all `attach_calling` call-site functions beyond those cited.
- Test-only `callback_body_controls.rs` beyond grep hits.
- Commit `457adf4a11f` line-for-line verification (read current tree paths under `omega-rust/omega/compilation/realization/`; citations match current line numbers for opened regions).

SLICE COMPLETE
