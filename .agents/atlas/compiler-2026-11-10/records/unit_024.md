# Unit 24 notes

Reader 24 of 113. Checkout `<checkout>/` (main at `457adf4a11f`). Paths below are relative to `omega-rust/omega/compilation/realization/` unless they start with `omega-rust/` or another crate root. "read" = I read the lines; "grepped" = Grep across `omega-rust`; "inferred" = concluded without reading the other site.

Files covered (54, 13,640 lines):

provider-planning/src: program_entry/discovery.rs, program_entry/source_contract.rs, provider_declarations.rs, provider_identity.rs, provider_instances.rs, provider_planning.rs, provider_planning/external_binding_rows.rs, provider_planning/provenance_replay.rs, provider_planning/provenance_replay/adapter_conformance.rs, .../named_provider_plans.rs, .../plan_derivation.rs, provider_planning/provider_rows.rs, provider_planning/selected_declarations.rs, provider_planning/source_provider_selections.rs, provider_settlement.rs, provider_settlement/assumed_bindings.rs, provider_settlement/completion.rs, provider_settlement/semantic_roles.rs, sealed_accessors.rs, service_declarations.rs, service_schema/mod.rs, source_guards.rs, target_machines.rs, target_machines/prepared_providers.rs, target_machines/provider_publication.rs.
provider-selection/src: binding.rs, binding/installation_reach.rs, binding/provider_instances.rs, bodies.rs, bodies/callback_roots.rs, bodies/target_bodies.rs, boundary.rs, choice.rs, choice/declarations.rs, choice/independent_components.rs, choice/selection.rs, lib.rs, publication.rs.
selected-dispatch/src: compiler_intrinsic.rs, intrinsic_review.rs, lib.rs, numeric_provider.rs, provider_binding.rs, selected_dispatch.rs, selected_dispatch/operator_adapter.rs, .../application_realization.rs, .../specialized_application_realization.rs, selected_execution.rs.
trust-model/src: accepted_machine_grants.rs, admission_settlement.rs, lib.rs, tests.rs, trust_admission.rs.
`omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/access_references.rs`.

How the pieces fit (read): provider-planning derives candidate `ProviderPlan`s with provenance from checked source (provenance_replay), prepares them with Calling plans (target_machines/prepared_providers), and settles (provider_settlement). provider-selection is source-free: it chooses one whole plan per slot (choice), grows demand to a fixed point against the Terminal artifact (publication), binds the chosen plans (binding) and admits boundary use (boundary). selected-dispatch turns the settled selection into `SelectedExecution` (provider_binding) and joins operator uses to checked bodies. trust-model turns root grants into `TrustAdmission`s.

---------------------------------------------------------------------
## provider-planning/src/program_entry/discovery.rs (100 lines)

1. Purpose. Nominates, for package review, the one package-owned `ProgramEntry` boundary schema that an explicit provider selection names, producing an `AcceptedSemanticBinding` candidate. Header (1-7) says it is nomination only; ordinary compilation still needs the accepted binding.
2. Shape. `discover_explicit_entry_binding` (16) is the only function.
3. In/out. Reads `ServiceCatalog`, `RootBinding`s, target, `ProviderSelection`s, `PackageCompilationInputs`; returns `Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>>`.
4. Decides. Filters traits where `definition.is_boundary && definition.name.as_str() == schema_name` (48-51: spelling of the schema name plus `context.contains`). A schema is "explicitly selected" if a `BoundaryRequirement` selection's `identity.symbol == requirement.signature` (65), or a `BoundaryTrait` selection's trait owns a signature with that symbol (67-74); `BoundaryOperatorFamily` never matches (75). Two matches is an error (80-84).
5. Shapes. Exactly one schema; entry slot must have all three of package/boundary_schema/physical_arrival_requirement else `Ok(None)` (36-42).
6. Same job elsewhere. `source_contract.rs:133 selected_entry_schema` also finds the entry schema by name inside a contract owner (different selection key: source digest / accepted binding).
7. Callers. `discover_explicit_entry_binding`: 1 call site, `omega/compilation/compiler/src/checked/target_preparation.rs:49` (grepped).
8. Stated limits. "This is nomination only" (3).
9. Fail-open. `Ok(None)` when `!inputs.semantic_role_discovery()` (30), when no entry machine (33), when slot fields missing (36-42), or when an accepted binding already exists (44): all return "no candidate", not acceptance. A schema whose `owner` has no package root or whose `arrival_requirement_contract` errs is silently skipped (53-62). None accepts anything; stated by header (nomination only).

## provider-planning/src/program_entry/source_contract.rs (364 lines)

1. Purpose. Freezes the entry declaration facts (schema, semantic and physical requirement, calling conventions, parameter and result type identities) before target Calling settlement, and later validates that the physical contract source is the exact bundled source or the exact accepted package binding.
2. Shape. `PreparedEntryCallingContract` (21), `PreparedEntrySurface` (37), `prepare` (42), `selected_entry_schema` (133), `TargetOwnedPhysicalContractSource` (216), `PreparedPhysicalContractSource` (222), `capture_physical_contract_source` (234), `PreparedPhysicalContractSource::validate` (295), `physical_contract_package_source_report_fingerprint` (354, FNV1a over domain, identity, source).
3. In/out. Reads typed trees via `ServiceCatalog`, the slot declaration (`target::ProgramEntrySlotDeclaration`), optional `AcceptedSemanticBinding`; outputs the prepared contract with deferred `surface: Result<..>` and type identity strings.
4. Decides. Schema choice (133-214): with an accepted binding, matches traits whose package == `binding.package()` and `display_path == binding.declaration_path()` and only if `selected_provider_plan_digest().is_none()` (145-156); without one, matches traits by name `definition.name.as_str() == schema_name` (173) inside source files whose bytes hash equals the target-owned digest (160-169, `exact_physical_contract_package_source_digest`). `validate` (295-352) keys on package identity equality plus `schema_path == binding.declaration_path()` plus a schema digest (305-315) or on bundled-source custody (324-333).
5. Shapes. Requires exactly one schema (197-204). Four-field slot declaration is all-or-nothing (48-72). Semantic and physical requirement must differ by signature and identity (80-89).
6. Same job elsewhere. Schema-by-name lookup also in `discovery.rs:48-52`.
7. Callers. `prepare`, `PreparedPhysicalContractSource::validate` are `pub(super)`; users are in the sibling `program_entry.rs` (not read, inferred).
8. Stated limits. Header (1-8): "package bindings retain the complete declaration path and never fall back to the bundled contract".
9. Fail-open. (a) `typed.type_identity_text(parameter.identity).unwrap_or_default()` (121-124) and 128: a missing type identity becomes an empty string in `parameter_type_identities`; traced no further (open: what consumes an empty identity is outside this file). (b) `validate` `None` arm accepts when `source_package.is_none()` etc., i.e. a bundled copy with the matching digest; this is the intended bundled path (stated by the error text 336 "exact bundled ... contract or one accepted package-owned ... binding").

## provider-planning/src/provider_declarations.rs (103 lines)

1. Purpose. Closes the nominal declaration identity of the selecting provider type and of row-aligned numeric-intrinsic realizations while source custody exists, giving `TerminalTargetProviderPlan`.
2. Shape. `close_selecting_declaration` (11), `close_nominal_declaration` (22), `close_provider_declarations` (38), `retain_selected_provider_declarations` (79).
3. In/out. Reads `ProviderPlan`, `ProviderPlanProvenance`, `exact_toolchain_sources`; returns `TerminalTargetProviderPlan { plan, provider_declaration, row_numeric_realizations }`.
4. Decides. A row gets a numeric realization only if `row.binding` is `CompilerIntrinsic` and `typed.numeric_intrinsic_execution(requirement).is_some()` (61-64), then `realization.authored().map(nominal)` (66).
5. Shapes. none.
6. Same job elsewhere. `close_nominal_declaration` (22-36) and `provider_publication.rs:278-289 declaration_identity` both do `exact_owner_nominal_identity(...).and_then(type_identity_text)` with different error strings.
7. Callers. `retain_selected_provider_declarations`: 1 (`omega/build-system/build-settings/src/admission/provider_execution.rs:39`). `close_provider_declarations`: used in `provider_publication.rs:137` and here. (grepped)
8. Limits. none.
9. Fail-open. none seen (length mismatches return errors 46-51, 85-89, 95-98).

## provider-planning/src/provider_identity.rs (260 lines)

1. Purpose. Decides whether a typed machine realizes a given provider plan row: finds the named provider implementation (`Conformance`) for a plan, its row, and the trait application.
2. Shape. `named_provider` (16), `ProviderTraitApplication` enum Named/Machine (32), `provider_trait_application` (51), `provider_trait_requirement_matches` (114), `named_provider_row` (130), `provider_row_matches` (180).
3. In/out. Reads `typed.conformances()`, machine trait conformances, `ProviderPlan` fields (`provider_type`, `provider_type_package_identity`, `application_identity()`, `identities`); returns refs or bool.
4. Decides. `named_provider` (21-29): conformance whose `display_path(symbol,"::") == plan.provider_type` and package identity equals plan's, `eligible(definition)` unless plan has an application identity (22); exactly one match else `None`. `named_provider_row` (137-177): row matches by own callable-identity text equality (147) or by satisfaction-edge identity text (167-172), exactly one. `provider_row_matches` (180-260): named case checks realization machine's first state symbol, `realized_row_machine`, instantiated-state match, and the implementation-capture identity text vs `plan.application_identity()` (225-250); non-named case matches `attached_data_path(machine) == plan.provider_type` plus package (252-258). All joins are display-path/identity text plus package identity.
5. Shapes. none (counts must be exactly one).
6. Same job elsewhere. Own-identity computation at 138-147 is repeated at `provenance_replay/adapter_conformance.rs:253-266`; requirement callable-identity text from (trait, requirement) is also built in `provenance_replay.rs:158-181 exact_satisfied_requirement_identity` and `provider_settlement/assumed_bindings.rs:74-92`.
7. Callers (grepped). `provider_trait_requirement_matches`: 2 call sites, both in test support `compilation/compiler/tests/support/checked_interpreter_dispatch/boundary_dispatch/adapter_rows.rs:294,658`; 0 production callers (plus the re-export at provider-planning `lib.rs:38`). `provider_row_matches` has production callers at `adapter_conformance.rs:233` and in this file.
8. Limits. none.
9. Fail-open. `eligible(definition)` is skipped when the plan has an application identity (22): a conformance that is not `eligible` can then match; traced no further (open). Otherwise `None`/false on ambiguity.

## provider-planning/src/provider_instances.rs (291 lines)

1. Purpose. `SelectedProviderSource` holds the checked program, provider inventory and selected plans for explicit inspection / checked interpreter; `project_instances` attaches exact checked provider instances to the selected plans.
2. Shape. `SelectedProviderSource` (13; `declarations: OnceLock<PreparedProviderInstances>`), `project_instances` (61), `CheckedAdapterDeclarations` (98), `prepare_instance_declarations` (141), `render_instance` (281).
3. In/out. Reads `typed`, `check_facts`, plans; produces `provider_selection::ProviderPlanInstanceDeclarations` per plan then calls `provider_selection::project_provider_instances` (75).
4. Decides. Equality of two sources is `Arc::ptr_eq` on the program plus plan equality (34, header 2-4). Rows: `CompilerIntrinsic`/`Syscall` use `closed_provider_leaf_applications(typed, template)` (175-196), `CheckedAdapter` uses `inherited_instances` via the prepared adapter lookup keyed by `std::ptr::eq` of plan and row (130), all other bindings produce an empty list (229). Generic templates are included when `*schema == plan.schema` (235).
5. Shapes. Only top-level requirement machines with type parameters become templates (157-161).
6. Same job elsewhere. `binding/provider_instances.rs` (provider-selection) consumes the result.
7. Callers. `SelectedProviderSource::new`: 1 production site (`compiler/src/execution_settlement.rs:454`) (grepped).
8. Limits. Header: "Preparing declarations neither reports deferred failures nor admits the projected provider closure."
9. Fail-open. `_ => Ok(Vec::new())` (229, 256): rows with `Import`, `VtableField`, `TableFunction`, `VtableSlot` get no inherited instances. Traced only to the consumer (`binding/provider_instances.rs:64-72` also `continue`s for those). Whether such rows can carry closed requirement instances is not checked here (open).

## provider-planning/src/provider_planning.rs (365 lines)

1. Purpose. Module root for candidate derivation and selection adapters; holds the provenance enums and the inline `selection_declarations` module that projects between `ProviderSelection` (source) and `provider_selection` canonical records.
2. Shape. `ProviderSelectionProvenance` (65), `SelectedProviderPlanWithProvenance` (79), `SelectedProviderReviewProvenance` (85; fields `row_compiler_intrinsic_executions`, `row_external_completion_leaves`), `selected_provider_plan_facts` (93), `selected_provider_plan_facts_with_independent_components` (106), `select_derived_provider_plans` (158), `entry_state` (205), `selection_declarations::{project_declaration, retain_family_replay, project_source_declaration, project_provenance, restore_provenance, source_candidate}` (212-364).
3. In/out. Source candidates in, `provider_selection::choose` / `admit` out.
4. Decides. `source_candidate` marks `native_build = row_realizations.iter().all(|row| row.native().is_some())` (347-351). `selecting_declaration` starts as an `Err("canonical selecting declaration has not been prepared")` (357) and is overwritten (138).
5. Shapes. none.
6. Same job elsewhere. `native_build` is recomputed the same way at `target_machines/provider_publication.rs:157-161`.
7. Callers (grepped). `selected_provider_plan_facts`: 1 (`build-settings/src/admission/checking.rs:154`). `select_derived_provider_plans`: 1 (`checking.rs:148`). `_with_independent_components`: 1 (the wrapper at 103, passing empty component and toolchain slices) and a README mention; so the independent-component path is only reached with empty lists from the single caller of the wrapper (inferred; other pub(crate) callers: none found).
8. Limits. none.
9. Fail-open. `native_build` is true when `row_realizations` is empty (`all` on empty). Traced: derived plans in this unit always push a row immediately (`plan_derivation.rs:143,153`, `named_provider_plans.rs:243`, accessors 663-680, and `rows.first()?` at named:70), so unreachable inside the unit; `native_bindings/plans.rs:164-178` builds plans too and was not read (suspected). A native_build candidate gets `SelectionProvenance::NativeBuild` (`choice.rs:67-69`) and skips correspondence checks (`choice.rs:126-128`).

## provider-planning/src/provider_planning/external_binding_rows.rs (314 lines)

1. Purpose. Turns selected plan rows that are `Import`, `Syscall`, `VtableField` or `TableFunction` into `calling_conventions::ExternalBindingRow`s, computing dispatch-table field byte offsets from the foreign record layout.
2. Shape. `extract_native_external_binding_rows` (15), `ExternalBindingContext` (41) with `target_profile` (50) and `row` (84), `field_offset` (211).
3. In/out. Reads plans, `BoundaryCallingPlanRealization`s, `CallingProgram` artifact module (structural types, native layouts, ABI signatures); outputs rows with `calling_applications`.
4. Decides. Skips `CompilerIntrinsic | VtableSlot | CheckedAdapter` (103-109). `field_offset` finds the table record by identity text == `storage.table_application` and the field by `declaration_identity == storage.field_declaration` (219-247), each requiring exactly one; field must be a live, address-typed scalar (253-258); every selected ABI must give the same offset (308-310).
5. Shapes. Hard-coded: table dispatch field "must have type addr" (257); formal zero must be the exact readable table borrow (274-283).
6. Same job elsewhere. none seen.
7. Callers. `extract_native_external_binding_rows`: 1 (`provider_settlement.rs:230`). `ExternalBindingContext::row` also from `provider_rows.rs:335`.
8. Limits. none.
9. Fail-open. (a) For `Syscall`, `compatibility_policy = native_syscall_for_target(native_target)` is an `Option` (167-169); if `None` and no explicit applications exist, the row is emitted with empty `calling_applications` (172-194). Open: what a target without a syscall policy does with a Syscall row is not visible here. (b) Fallback `selected_target.unwrap_or("cross_platform_cli")` (196-197) is unreachable because `target_profile` errors first when both names are absent (71-74): traced.

## provider-planning/src/provider_planning/provenance_replay.rs (517 lines)

1. Purpose. Owns the provenance carriers and derivation entry point for candidate provider plans, plus the helper module `requirement_identities` (149-517) that recognizes inferable compiler leaves and builds bindings.
2. Shape. `ProviderRowRealization` Authored/BuildNative/Toolchain (30), `ProviderPlanProvenance` (52: `schema`, `provider_type`, `named_application`, `row_requirements`, `row_realizations`, `row_target_machine_origins`, `row_accessors`), `DerivedProviderPlan` (80), `ProviderPlanDerivation` (86, built by `unevaluated` 97 / `evaluated` 114), `derive_satisfies_plans` (135); in `requirement_identities`: `exact_satisfied_requirement_identity` (158), `exact_installed_external_binding_identity` (204), `inferred_hosted_catalog_leaf` (226), `inferred_hosted_console_compiler_intrinsic` (285), `inferred_selected_target_compiler_leaf` (357), `inferred_compiler_leaf_binding` (393), `exact_catalog_signature` (418), `external_provider_binding` (452), `satisfies_plan_name` (494), `sole_machine_origin` (503).
3. In/out. typed trees + `SelectedTargetMachineOrigin`s in; `ProviderBinding` and origin out.
4. Decides. By source spelling: `inferred_hosted_catalog_leaf` (226-261) matches `machine_name` literally against `"ConsoleNativeProvider::exit_process"`, `"ConsoleNativeProvider::write_byte"`, `"ConsoleNativeProvider::read_byte"`, `"ProcessExitNativeProvider::exit_process"` and returns fixed (provider, trait, requirement, shape). The hosted recognizer then also compares `attached_data` to the provider name (299), `conformance.name` to the trait name (302), requirement name (303), `definition.name` (318), `requirement.name` (321) and the machine-name prefix (322-327); and an exact catalog signature (328-349). Non-hosted targets: any boundary machine with `attached_data`, no body, one state, and exactly one origin for the selected target becomes a `CompilerIntrinsic` leaf (357-391) with no signature check. External bindings join by `same_semantic_name` (192, string equality helper from `symbols`).
5. Cases by shape. `exact_catalog_signature` (418-435): `I32ToUnit` (one non-const non-mut `i32` parameter, unit result) and `UnitToByteRead`; `UnitToAddress | ProgramStorageToUnit => false` (432-433). The four-name table above is a closed list.
6. Same job elsewhere. `exact_satisfied_requirement_identity` duplicates the (trait, requirement) -> callable-identity text job also done in `provider_identity.rs:138-147` and `assumed_bindings.rs:74-92`. Hosted leaf names are also hard-coded in `intrinsic_declarations.rs` (file list only, grepped `ConsoleNativeProvider|ProcessExitNativeProvider`: 2 files, `provenance_replay.rs` and `intrinsic_declarations.rs`; suspected, not read).
7. Single-producer claim. Header 8-9: "Derivation is the single producer of a `ProviderPlan` and its `ProviderPlanProvenance`". Grepped `ProviderPlanProvenance {`: constructed at `plan_derivation.rs:593`, `named_provider_plans.rs:90` and `omega/compilation/realization/provider-planning/src/native_bindings/plans.rs:178` (outside this file, not read). `ProviderPlan {` is also built at `native_bindings/plans.rs:165`. So production sites: 3 for provenance, 3 for plan (plan_derivation:580, named:72, native_bindings:165).
8. Limits. Header: "adapter refinement of a requirement's service and invocation ceilings belongs to Psi conformance checking".
9. Fail-open. `inferred_selected_target_compiler_leaf` (357-391): any non-hosted target, Boundary-supply, bodiless, attached-data machine with one state and a matching target origin is bound to `CompilerIntrinsic` without checking its signature or catalog name. Open: which wrong program passes depends on whether the later `IntrinsicDeclaration::settle` (not read) rejects unknown machines. `exact_satisfied_requirement_identity` returns `String::new()` when the trait is missing (175-176) and `unwrap_or_default` otherwise; traced: callers in `named_provider_plans.rs:154` and `plan_derivation.rs:112` intern the empty string as a requirement identity with no check (open).

## provider-planning/src/provider_planning/provenance_replay/adapter_conformance.rs (353 lines)

1. Purpose. Resolves the exact checked adapter machine realizing a checked-adapter row, verifying identity, package, ownership, satisfies-edge scope and requirement lifetime partition.
2. Shape. `selected_requirement_checked_adapter` (13), `exact_checked_adapter` (107), `validate_exact_requirement_lifetime_partition` (246); re-exports `exact_row_for_schema_method` from backend-plan (11).
3. In/out. typed trees, `ProviderPlan`, row; returns `&Machine` or `Diagnostic`.
4. Decides. Callable identity text from the plan -> typed machines with that identity that realize the plan target (139-159), then package identity filter (160-166); exactly one (167-193). Then context membership of attached data and every satisfies edge (201-223), package re-check (224-231), `provider_row_matches` when the plan has a provider type (232-241), and lifetime partition equality (246-353).
5. Cases by shape. `selected_requirement_checked_adapter` requires the schema to identify exactly one method (62-66) and a plan with exactly the schema's method count (74); returns `Ok(None)` unless the row is a `CheckedAdapter` (85-87).
6. Same job elsewhere. Operator-specific variant: `selected-dispatch/.../operator_adapter.rs:27-127 resolve_checked_adapter_for_operator` (also "one method, one row, overload identity, checked-adapter, provider type non-empty").
7. Callers (grepped). `exact_checked_adapter`: production callers `adapter_conformance.rs:94`, `provider_instances.rs:115`; test support `adapter_rows.rs:773`, `callback_body_controls.rs:118`; a source-text guard `compiler/src/product_boundary.rs:679` matches the string `exact_checked_adapter(context, plan, row)`. `selected_requirement_checked_adapter`: 1 production caller (`source_provider_selections.rs:22`).
8. Limits. none.
9. Fail-open. `matching.as_slice()` `[] if row.requirement_lifetime_partition.is_empty() => Ok(())` (328): a realization with no satisfying edge is accepted when the row's partition is empty. Traced: that equals "no lifetime args recorded", so no partition to disagree; stated by the structure of the error cases at 336-340 only. Line 232: the provider-row join is skipped when `plan.provider_type.is_empty()` (open: plans from top-level requirements always have a provider type in this unit, `plan_derivation.rs:235`).

## provider-planning/src/provider_planning/provenance_replay/named_provider_plans.rs (267 lines)

1. Purpose. Derives whole-plan candidates for named provider implementations (`Conformance` maps) from prepared named applications, joining foreign/build-owned member rows to the candidate inventory.
2. Shape. `derive` (21), one big `filter_map` closure.
3. In/out. `ServiceCatalog`, calling plans, selected target, candidate inventory in; `Vec<DerivedProviderPlan>` out.
4. Decides. Each `?`/`return None` silently drops the whole map (109, 115, 121, 132, 146, 152, 164, 211-213, 217, 221); only `OMEGA_PROVIDER_DEBUG` prints a reason (29, 31, 40, 52...). Row binding: a checked-body machine becomes `CheckedAdapter` (165-184); otherwise it must match, by authored symbol or build-native `origin.requirement` plus an edge for that requirement, a candidate row with equal requirement identity text (186-216), and all matches must agree (218-222).
5. Shapes. All rows of one map must realize in one origin package (148-153).
6. Same job elsewhere. The `ProviderPlan { .. }` literal (72-84) duplicates the one in `plan_derivation.rs:580-592`.
7. Callers. Called from `plan_derivation.rs:173`.
8. Limits. none.
9. Fail-open. Not fail-open (fails closed by dropping candidates) but note: a map dropped silently can turn into "no candidate" and later a missing-selection error with no pointer to the cause; debugging env var read at 29 (`std::env::var_os("OMEGA_PROVIDER_DEBUG")`; grepped count 3 across omega-rust, all in this folder).

## provider-planning/src/provider_planning/provenance_replay/plan_derivation.rs (685 lines)

1. Purpose. Derives candidate plans from authored satisfaction declarations: trait-satisfaction plans, boundary-operator plans, top-level requirement plans, named-provider plans and sealed-accessor plans; then filters plans that reach absent target bodies.
2. Shape. `derive_provider_plans` (25), `derive_top_level_requirement_plans` (208), `provider_plan_schema_target` (319), `derive_boundary_operator_plans` (330), `DraftRowBinding` (430), `is_authored_provider_machine` (453), `checked_adapter_draft` (463), `evaluated_via_binding` (475), `external_realization_row_binding` (491), `mint_plan_row_identity` (550), `derived_plan_position` (558), `derive_accessor_plans` (607).
3. In/out. typed trees, `DirectCallDeclarations`, evaluated via-bindings table, target machine origins, native bindings in; `Vec<DerivedProviderPlan>` out.
4. Decides. Plan grouping key: plan name + provider-type package + origin package + schema declaration + provider-type symbol (571-577). Binding by (supply mode, external binding) match at 66-108: bodiless `Boundary` -> inferred compiler leaf unless the requirement symbol already has a native build row (72-74); `CheckedBody` with body -> `CheckedAdapter` only if the clause's trait is a boundary trait (89-95); else external realization binding. Machines that are specializations are skipped by `is_authored_provider_machine` (453-461, a receipt scan). Absent-body filter at 186-204 drops any plan with an authored realization in `DirectTargetBodyAbsences`; the target for that is `selected_target` or `TargetProfile::host().target_name()` (186-191).
5. Shapes. Top-level requirement plan requires exactly one schema method (260-262). Boundary-operator plan row method name is hard-coded `"realize"` (416). Accessor plan rows are always `CompilerIntrinsic` with the placed-scalar identity (668-672).
6. Same job elsewhere. Three near-identical loops (satisfies / top-level / operator) each repeat the realize-check, binding and `derived_plan_position` sequence (51-159, 208-317, 330-428). Plan literal at 579-602 also in `named_provider_plans.rs:72`.
7. Callers. `derive_provider_plans`: 1 (`provenance_replay.rs:138`).
8. Limits. none stated.
9. Fail-open. (a) `selected_target == None` falls back to the host target name for the absence filter (190). Traced: `derive_satisfies_plans` with `None` target happens only if the caller passes none; the settle path uses `host_if_supported` (`provider_settlement.rs:168`), so the fallback is reached only for unsupported hosts (inferred, not read further). (b) `external_realization_row_binding` `evaluated_via_binding` returns `None` unless the table target equals the selected target (483-486); `None` drops the clause (continue at 104) rather than accepting. (c) `derive_boundary_operator_plans` uses `schema.declaration.methods[0]` indexing (413) with no length check; panic risk, not an accept.

## provider-planning/src/provider_planning/provider_rows.rs (441 lines)

1. Purpose. Captures, while source exists, per-row declaration facts for direct boundary calls and intrinsic/exit-process declarations, keeping each paired with its original candidate; defers errors until the candidate is selected.
2. Shape. `DeferredProviderRows` (13), `CapturedProviderRows` (18), `RowDeclarations` (24), `DirectBoundaryDeclaration` (30), `PreparedProviderRows` (40), `ProviderPlanCandidate<Rows>` (58), `DirectBoundarySource` (137), `DirectBoundaryMechanism` Intrinsic/Foreign (147), `DirectBoundaryInput` (153) with `finish` (163), `capture_direct_declarations` (213), `CapturedProviderRows::{capture, attach_physical_rows}` (305-364), `capture_row_declarations` (366), `finish_row_declarations` (421).
3. In/out. `DerivedProviderPlan` + `ServiceCatalog` in; candidates with direct inputs and `ProviderRowDeclarations { intrinsic, exit_process }` out.
4. Decides. A direct leaf exists only for rows with binding in {CompilerIntrinsic, Import, Syscall, VtableField, TableFunction} (220-228) whose machine is found (authored symbol, or for `BuildNative` the first typed machine with no body, an edge for `origin.requirement`, realizing the target: 235-248) and has no body (253). Exactly one schema method must carry the row's requirement identity (264-283). `matches_selection` compares plan and provenance by equality (94-96).
5. Shapes. `declaration_identity`: `normalized_hermetic_symbol_identity` else for intrinsics `"program-local::{display_path}"` (285-292). `finish` (163-211) rewrites the plan to a one-row `direct-boundary:{target}:{callable}` plan.
6. Same job elsewhere. none.
7. Callers (inferred, not read): `ProviderPlanCandidate::direct_boundaries`, `row_declarations`, `direct_declarations`, `accessor` used by `selected-dispatch/intrinsic_review.rs:143,189,334-352`.
8. Limits. Header 4-6: intrinsic and exit-process errors "wait until after `ProgramEntry` checks".
9. Fail-open. `let Some(machine) = ... else { continue; }` (250-252) and `if machine.body_is_present { continue; }` (253-255): a direct-boundary row whose realization machine cannot be found, or which has a body, produces no direct leaf and no error. Open (not traced). The `program-local::` fallback identity for intrinsic rows (287-290) is explicit in code.

## provider-planning/src/provider_planning/selected_declarations.rs (101 lines)

1. Purpose. Projects each selected plan to source-free `ProviderDeclarations` (installation reach, leaf applications) via `checked.provider_declarations`, then aligns them to selected plans.
2. Shape. `capture_plan_declarations` (15), `prepare_selected_plan_declarations` (69).
3. In/out. `CheckedTrees`, plan, provenance in; `ProviderDeclarations` out; wrapped `SelectedPlanDeclarations` out.
4. Decides. Application view: `CompilerIntrinsic | Syscall` -> `PhysicalLeaf`, `Import` -> `External`, else `None` (48-54). `installation_reach` is requested unless the schema is `BoundaryOperator` or `SealedAccessor` (60-64).
5. Shapes. none.
6. Callers. `capture_plan_declarations`: from `prepared_providers.rs:469`; `prepare_selected_plan_declarations`: 1 (`selected-dispatch/provider_binding.rs:150`).
7-9. Limits none. Fail-open none seen (length/equality mismatches return errors 20-26, 76-79, 87-93).

## provider-planning/src/provider_planning/source_provider_selections.rs (186 lines)

1. Purpose. Early/reference adapters that locate the selected plan for a checked use of a boundary operator, trait method or top-level requirement, prepare build-time provider bodies and project checked operator body candidates.
2. Shape. `prepare_selected_build_time_provider_body` (13), `selected_use_plan` (44), `selected_requirement_plan` (74), `use_site_is_generic_template` (140), `provider_type_package_identity`/`provider_type_symbol` (152-167), `selected_checked_operator_body` (169), `provider_type_path` (181).
3. Decides. `selected_requirement_plan`: trait-method requirements select plans whose schema binds the exact boundary trait and has exactly one method with the requirement's identity text, exactly one such plan (83-103); operators select plans whose `trait_name_text == slot` (string) and equal package (109-120); other requirements go through `IntrinsicRequirement::by_symbol` and must be `TopLevelRequirement` (123-131); result requires exactly one match (134-137). `selected_use_plan` additionally returns `None` for a single-row `CheckedAdapter` operator plan used inside a generic machine template (61-70).
4. Shapes. Operator plans are matched by overload-identity string equality to `trait_name_text` (117).
5. Same job elsewhere. Schema-to-requirement binding is also in `service_schema/mod.rs:85-142` and `provider-selection/boundary.rs:352-357`.
6. Callers (grepped). `selected_use_plan`: `operator_adapter.rs:155` (selected-dispatch), `review-evidence .../compiler_intrinsics.rs:189,202`. `selected_requirement_plan`: `selected_constants.rs:81`, `compiler_intrinsics.rs:368` and the internal call at 56.
7. Fail-open. `None` returns mean "no plan"; consumers treat `None` as "not selected" (see `application_realization.rs:197-204` returning `Ok(None)`). Open: whether every such `None` is later reported. `selected_use_plan` for a missing machine returns `None` (51-55).

## provider-planning/src/provider_settlement.rs (434 lines)

1. Purpose. Settles which provider each boundary API uses for the target: attaches Calling plans, verifies independent component descriptions, runs whole-map selection against the Terminal artifact, validates synchronous invocation cycles, extracts external binding rows, validates assumed bindings and completion roles.
2. Shape. `SettledTargetMachineDeclarations` (45), `CheckedProviderSelection` (50), `ProviderSettlement<Rows>` (61), `finish_row_declarations` (82), `calling_applications` (127), `settle_checked_providers` (160-359), `verify_selected_compositions_are_realized` (361), `verification_request` (385), `verify_independent_component_descriptions` (396), `verify_independent_components` (422).
3. In/out. `PreparedProductProviders`, package inputs, accepted component assumptions, Calling realizations, `CallingProgram` in; `CheckedProviderSelection` out.
4. Decides. Planning target = selected profile or `host_if_supported` (168). Completion-role validation only runs if `package_inputs.semantic_role_discovery()` (263). Assumed bindings: each `required` appears once and must not have an explicit selected row (279-290); the supplier must have a selected row (292-304). Independent component descriptions are verified with schema set `{COMPONENT_DESCRIPTION_SCHEMA_V3}` (391).
5. Shapes. `verify_selected_compositions_are_realized` rejects any `Independent` edge outright (368-371): "the product defines no symbolic import/export, entry/leave, or installation/replacement carrier".
6. Same job elsewhere. none.
7. Callers (grepped). `verify_selected_compositions_are_realized`: 1 (`compiler/src/terminal/terminal_artifact.rs:510`). `verify_independent_component_descriptions`: pub(crate), 1 caller (line 429, same file). Guard: `compiler/src/product_boundary.rs:697` reads this file with `include_str!` and bans two strings (`collect_program_references(`, `provider_closed_requirements`).
8. Limits. Header 20-23 (quoted above): independent edges refused until a carrier exists.
9. Fail-open. `package_inputs == None` yields no components (426-428); an `Independent` selection then fails in `IndependentComponentJoin::realize` ("no verified component", `independent_components.rs:42-47`): traced, fails closed. `completion_roles.validate` skipped without `semantic_role_discovery` (263-265): stated by the role-discovery gate; a program not in discovery mode is not checked for the exit-role (open whether another check covers it).

## provider-planning/src/provider_settlement/assumed_bindings.rs (191 lines)

1. Purpose. Checks that two boundary requirements joined by an `AssumedBindingContract` are compatible in source signature, operational/lifetime envelope and lifetime subject mapping, then later in validated layout/calling plan.
2. Shape. `PreparedAssumedBindings` (13), `PreparedAssumedBinding` (18), `capture` (24), `validate_calling` (39), `coordinates` (70), `source_compatibility` (104).
3. Decides. Parameter count and `normalized_type_identity` equality for return and each parameter (122-132); `suspends`/`blocks` may not be strengthened and `termination_guarantee` must be equal, `invokes` must be empty on both sides, equal lifetime-parameter counts (135-142); lifetime mapping is positional (146-151), `applied_type_identity_in_scope` equality with `required.is_some()` (159-174). Calling check: both sides' `exact_boundary_entry_plan()` and `materialized_signature()` equal (53-54).
4. Shapes. `invokes` must be empty on both sides: "incompatible or unsupported operational/lifetime correspondence" (143).
5. Same job elsewhere. `coordinates` (74-92) and `source_compatibility`'s `signature` (108-115) both scan `typed.traits()` for a signature symbol; the callable-identity text job is also at `provider_identity.rs:138-147`.
6. Callers. `PreparedAssumedBindings::capture`: `prepared_providers.rs:503`; `validate_calling`: `provider_settlement.rs:277`.
7. Fail-open. (a) `(None, None) => true` (51): if neither required nor supplied has a Calling plan realization the compatibility check passes. Open (no comment): what program has two assumed-bound requirements with no calling rows; source compatibility (above) still ran. (b) The `normalized_type_identity(..) != ..` check at 123-131 treats two `None`s as equal, but the second check at 173 requires `required.is_some()`, so traced: closed.

## provider-planning/src/provider_settlement/completion.rs (767 lines)

1. Purpose. Verifies that a provider satisfying an external-completion requirement reaches, on every checked execution path, an original external completion leaf (`CompilerIntrinsic` or `Import`) with the status argument `Some(0)` preserved; normal returns, crashes, cycles, redispatch reject.
2. Shape. `capture` (18), `Values` (61), `Outcome` (84, `returns`, `result: Origin`, `leaves`), `checked_completion` (115), `CompletionReview` (136) with `machine` (145), `operations` (282), `operation` (298), `scalar` (336), `scalar_invocation` (479), `scalar_destination` (514), `scalar_terminator` (557), `successor` (619), `arm` (655), `terminator` (705).
3. In/out. `CheckedTrees` flow facts (terminal unit effects, scalar computations, guarded exits) and candidates in; `Vec<Vec<SymbolHandle>>` (leaves per row) out.
4. Decides. A bodiless callee counts as a leaf only if some candidate row has an authored realization equal to that machine, or a build-native realization with an edge for the origin requirement, the same requirement symbol, and binding `CompilerIntrinsic|Import` (157-194); the argument vector must equal `[Some(0)]` (198-203). Abstract values are `Option<usize>` origins (16), so only parameter 0 flows (`Values::pure` 68-80). Cycles by an `active` stack of (machine, state) (233-237).
5. Cases by shape. Hard-coded rejection list: call with structural args or `Dynamic` route (301-307); operations other than `Call`/`EstablishLocal(Scalar)`/`Complete` ("no evaluation rule", 332); scalar computations other than Value/Qualification/Call/Apply/Select (403-406); calls with structural args, temporary cleanups or claim transfers (495-502); crash arms/terminators (525, 574, 685, 762).
6. Same job elsewhere. The "bodiless realization is a leaf" join (167-182) mirrors `named_provider_plans.rs:189-201` and `provider_rows.rs:231-250`.
7. Callers. `capture`: `prepared_providers.rs:460`; `checked_completion` is `pub(super)` (115), caller is `capture` only.
8. Limits. Header: "Normal returns, crashes, cycles, or service redispatches reject." Errors only reported "if the candidate is selected" (5-6).
9. Fail-open. (a) `capture` pushes an empty leaf list for any row whose realization is not `Authored` (30-33), so a `BuildNative`/`Toolchain` realization of an External-completion requirement is not reviewed. Open. (b) `arm` default `_ => Ok(Outcome::returning())` (701) and `scalar` returning `None` origin for unknown expressions both end in rejection (a normal-return path or `arguments != [Some(0)]` -> error 130-131, 198-203): traced, fails closed.

## provider-planning/src/provider_settlement/semantic_roles.rs (153 lines)

1. Purpose. For package review, captures and validates the declared exit-completion role: a program's `Console` exit API must belong to the declaring package and use an implementation reaching an external exit leaf.
2. Shape. `CompletionIdentity` (17), `PreparedCompletionRole::capture` (42), `PreparedCompletionRoles::{capture, validate}` (83-153).
3. Decides. By spelling: `trait_name_text(&plan.schema) != "Console"` (51) plus equal package identity across plan, schema, origin and owner (50-57). Eligible rows: requirement equals the declared completion requirement and realization's package is the same (63-73). `validate` requires at most one eligible row per plan to have external leaves (118-122), at most one declaration overall (127-131), and the set of source proposals whose owner appears as a selected `BoundaryTrait` schema must equal the discovered declarations (132-150).
4. Shapes. Only `Console` is a role here (51).
5. Same job elsewhere. Hosted `Console`/`ProcessExit` names appear in `provenance_replay.rs:234-260` and `selected-dispatch/intrinsic_review.rs:201-208` (`HostedService::Console|ProcessExit`).
6. Callers. `PreparedCompletionRole::capture`: `prepared_providers.rs:466`; `validate`: `provider_settlement.rs:264`.
7. Fail-open. `capture` returns `None` unless the plan is a `BoundaryTrait` named exactly `"Console"` and same-package (47-57); a differently named exit trait gets no role and is never validated here. Open.

## provider-planning/src/sealed_accessors.rs (191 lines)

1. Purpose. Compiler-catalog declarations for generated placed scalar accessors: identity and contract digest, rejoin of a plan to its accessor.
2. Shape. `SealedAccessor` (12), `binding_identity` (33), `format_binding_identity` (42; prefix `"compiler-catalog::placed-scalar.v1::"` + hex), `prepared_binding_identity` (51), `prepared_schema` (59), `prepared` (80), `execution_contract` (90), `schema_contract` (98), `rejoin_plan` (112), `authenticate` (179).
3. Decides. `rejoin_plan` treats a plan as an accessor plan if any `CompilerIntrinsic` row's machine string `starts_with("compiler-catalog::placed-scalar.v1::")` (116-120), requires one row (124-128), finds the machine whose callable identity text equals the row's requirement identity and authenticates it (130-143), requires exactly one, then equality of schema, provider path, packages and binding (157-175).
4. Same job elsewhere. The prefix literal exists only at 44 and 119 (grepped `compiler-catalog::placed-scalar`: 2 hits). Accessor plan production at `plan_derivation.rs:607-685`.
5. Callers (grepped). `authenticate`: `service_schema/mod.rs:148`, `review-evidence/.../provider_schema.rs:27`. `binding_identity`: used at line 151 (same file). `execution_contract`: `sealed_accessors.rs:38`, `intrinsic_declarations.rs:178`.
6. Fail-open. A plan with any matching-prefix row but another shape errors (124-128); a plan with no prefix row returns `Ok(None)` (121-123), i.e. is not treated as an accessor. A hand-spelled string with that prefix but no matching machine errors (144-148): fails closed.

## provider-planning/src/service_declarations.rs (172 lines)

1. Purpose. Attaches target Calling evidence to Source-closed service declarations and offers exact lookups of a service by package+path, symbol or nominal identity.
2. Shape. `ServiceDeclaration` (17), `ServiceDeclarations` (24), `SourceServiceDeclaration` (29), `prepare` (61), `select` (101), `for_symbol` (113), `identifies` (117), `rejoin` (141), `unique` (156).
3. Decides. `select` keys on `entry.package == Some(package) && entry.path == path` (109), a path string; `rejoin` keys on nominal identity string equality (146); all require exactly one (160-169), and `rejoin` also requires the retained declaration to equal the expected one (148).
4. Callers (inferred, not read). `selected-dispatch/compiler_intrinsic.rs:44,94`, `intrinsic_review.rs:75,283,358`, `provider_binding.rs:209,253`, `selected_execution.rs:107`.
5. Fail-open. `identifies` returns `Ok(false)` when the symbol is absent (122-124) and when the nominal identity differs (125-131); callers (not read) decide what false means; open.

## provider-planning/src/service_schema/mod.rs (167 lines)

1. Purpose. Attaches Calling plan realizations to Psi-projected service schemas, and decides whether a schema binds an exact boundary operator, trait or requirement.
2. Shape. `attach_calling` (18), `attach_requirement_calling` (35), `from_typed_operator` (69), `from_typed_boundary_requirement` (77), `schema_binds_exact_boundary_operator` (85), `schema_binds_exact_boundary_trait` (100), `schema_binds_exact_boundary_requirement` (115), `from_sealed_accessor` (144), re-export `is_product_declaration` (16).
3. Decides. Calling attachment only for `BoundaryTrait` schemas (44), matching `boundary_trait`, `boundary_arguments`, `argument_owner` and `requirement_machine` (47-51). Binding by trait name text + package identity (operator 93-97, trait 108-112); requirement binds by callable identity (131-135) or by `display_path == schema_trait_name` when the machine has no spelling (136-138).
4. Same job elsewhere. `provider-selection/boundary.rs:352-357 requirement_binds` and `selection.rs:14-22 provider_slot_key` use the same (package, trait-name text) pair.
5. Callers (grepped). `from_typed_operator`: 0 call sites (only its definition and the inner `projection::from_typed_operator`). `schema_binds_exact_boundary_operator` / `_requirement`: production `intrinsic_requirements.rs:68,74`, plus test support `adapter_rows.rs:107,140`. `is_product_declaration` re-export: only used by test support `boundary_fields.rs:8`; validation has its own private `is_product_declaration` (`psi/compilation/validation/src/declarations_core.rs:1641`) next to Psi's `typed-trees/src/service_declarations.rs:369` (suspected duplicate, not read).
6. Fail-open. Non-`BoundaryTrait` schemas get no calling attachments (44) by design; open for operator/requirement schemas whose Calling plans exist (they attach elsewhere, not read).

## provider-planning/src/source_guards.rs (91 lines)

1. Purpose. Test-only layering guards: Cargo.toml must not list `typed-trees`, `typed-trees-to-checked-trees`, `validation`; any line naming `typed_trees` must also name `checked_trees`; no source line names the conversion stage; three source-selection decoders may not be defined here.
2. Shape. four `#[test]` fns (14, 41, 58, 75), constant `CRATE_ROOT` (12).
3. Decides. Plain string containment on source lines (47, `source_audit::violations`).
4. Coverage of guard. Applies to this crate only; `selected-dispatch` imports `typed_trees::` directly (`selected_dispatch.rs:35`, `operator_adapter.rs:25`), outside this guard.
5. Fail-open. none; the guards are line-wise: a line naming `typed_trees` with `checked_trees` elsewhere on it passes (47).

## provider-planning/src/target_machines.rs (229 lines)

1. Purpose. Selects exact provider and target-machine members from Psi's retained declaration inventory: `AuthoredTargetProviders`, inert sibling call rejection, constant target bodies, realization predicates, target default settlement.
2. Shape. `AuthoredTargetProviders` (39; `target`, `defaults`, `candidates`) with `unevaluated` (46) and `derive` (70); `reject_reached_inert_sibling_calls` (107); `selected_constant_target_bodies` (133); `realized_row_machine` (169); `realizes_machine` (187); `settle_provider_defaults` (207).
3. Decides. `realizes_machine`: false for family interfaces or out-of-context symbols (192-194); in `Build` scope true iff `machine.target.is_none()` (195-197); otherwise true iff `source_target_context(machine)` is `None` or equals the target (the target defaults to the host's name when empty/None, 198-204). `realized_row_machine`: for a family interface picks `target_family_member(named, target)` (176-181). `settle_provider_defaults`: untargeted default rows apply to every target; targeted rows when `TargetProfile::from_canonical_target_name(name) == target` (212-219).
4. Shapes. none.
5. Same job elsewhere. `provider-selection/bodies/target_bodies.rs:160-176 realizes_provider_body` and `:129-142, 178-189 family_body/provider_body` implement the same two predicates over `DirectCallDeclarations` instead of typed trees (both read). `expect` guard `compiler/src/product_boundary.rs:717-732` reads this file by `include_str!` and bans `.machines()` and `ProviderPlanningContext` inside `settle_provider_defaults`.
6. Callers (grepped). `reject_reached_inert_sibling_calls`: 1 (`compiler/src/execution_settlement.rs:357`). `selected_constant_target_bodies`: 1 (`selected_constants.rs:74`). `settle_provider_defaults`: 4 non-definition call sites (`checking.rs:112`, `target_preparation.rs:101`, `calling_policy_plans/selected_providers.rs:50`, `prepared_providers.rs:434`).
7. Limits. Header 13-22: inert siblings "never enter an inert body"; reach goes "across explicit calls, named transitions, and selected provider rows".
8. Fail-open. (a) `realizes_machine`: a machine with no target context realizes for every target (203-204); that is the portable machine rule, not stated at the site. (b) Empty or `None` target silently becomes the host target (178-179, 198-200). (c) `reject_reached_inert_sibling_calls` passes `entry: Option` through; `execution_settlement.rs:360-362` passes `None` when no program entry was selected, and `reject_reached_inert_calls` then starts with an empty frontier (`target_bodies.rs:76-80`) and returns `Ok(())`: traced; no inert-call check for products without a selected entry.

## provider-planning/src/target_machines/prepared_providers.rs (518 lines)

1. Purpose. Collects candidate providers and their declarations before the shared intermediate program is published, then attaches target Calling plans: builds, per known target, a source proposal that is later attached to Calling realizations.
2. Shape. `PreparedProviderMetadata` (35), `ProductProviderPreparation` (47) with `capture` (345-517), `PreparedProductProviders` (65), `CallingCandidate` (87; `capture` 93, `attach` 119), `SourceProposal` (136), `SourcePublication` (176), `ProviderPublicationCatalog` (183; `prepare` 197), `SettledProductProviders` (244), `PreparedProductProviders::attach_calling` (255), `attach_choices` (328).
3. In/out. Reads checked trees/services, evaluated bindings, defaults, overrides, application roots, native bindings and assumed bindings; produces candidates, per-target proposals, completion metadata.
4. Decides. Ownership by `Weak::ptr_eq(&self.owner, &program.source_owner)` and equal planning target (261-266). Native-start index separates authored from native candidates (451, 278). The target catalog is `base_targets` plus every `TargetProfile::ALL` name (405-414); a target with no native realization or an unparseable name gets an `Err` row (419-429). `ProviderPublicationCatalog::prepare` (197-241): a missing target row is an error (229-231); the retained `Selected` choice is used only when `selected.profile.target_name() == target` (213-215), else the prepared choices with independent components (223-226).
5. Same job elsewhere. `attach` of Calling plans to schemas appears in both `CallingCandidate::attach` (119-133) and in `service_schema::attach_calling` (called from there).
6. Callers (inferred). `capture`: compiler (not read). `attach_calling`: `provider_settlement.rs:173`.
7. Limits. none besides header on digest memo clearing (130).
8. Fail-open. `choices.candidates.len() != candidates.len()` is an error (333-337). `derived.plan.digest_memo = Default::default()` is reset only when a surface exists (127-131); candidates with no surface keep a stale digest memo (inferred, `surface` is `None` for non-`BoundaryTrait` schemas at 113-115; the plan's schema is not changed there, so the memo remains valid): traced.

## provider-planning/src/target_machines/provider_publication.rs (290 lines)

1. Purpose. Closes authored selections into source-free per-target `PreparedTargetChoices`, and lists every method root of the selected interfaces.
2. Shape. `provider_selection_roots` (15), `TargetProviderPreparationRequest` (63; `application_roots: None` means reusable Source checking, `Some` the Build deployment roster, comment 66), `prepare_target` (71), `canonical_roots` (194), `canonical_choice` (213), `canonical_declaration` (233), `declaration_identity` (278).
3. Decides. Defaults and overrides are filtered by `choice.target.is_none_or(|selected| selected.canonical_name() == target)` (99-103). `canonical_declaration` classifies by `SymbolKind` (246-274); unsupported kinds -> error 274. `explicitly_selected` = any choice's subject `selects_plan(&Ok(schema), plan)` (134-136). Inherited trait owners come from the abstract trait scope visit (117-133).
4. Shapes. `SymbolKind` list: Trait, Conformance, Data, Domain, BuiltinType, Machine, Operator (247-273).
5. Same job elsewhere. `declaration_identity` (278-289) duplicates `provider_declarations.rs:22-36 close_nominal_declaration`.
6. Callers (grepped). `provider_selection_roots`: 1 (`compiler/src/terminal/product_code.rs:178`). Guard: `compiler/src/product_boundary.rs:706-713` bans the string `is_candidate_demanded` in this file.
7. Fail-open. `target::TargetProfile::from_canonical_target_name(target).ok() != Some(profile)` is an error (85-89). Roots filtered by scope string equality `scope == target` (203). None seen that accepts.

---------------------------------------------------------------------
## provider-selection/src/binding.rs (319 lines)

1. Purpose. Binds the selected plan facts: resolves provider and authored grants, validates qualification grants, admits boundary policy, then enriches plans with installation reach, physical-leaf aliases and external `Import` applications.
2. Shape. `SelectedPlanDeclarations` (36), `SelectedProviderPlanBinding` (42), `bind_selected_provider_plan_facts` (58), `validate_qualification_grants` (87), `enrich_selected_plans` (172).
3. Decides. Grants must replay against exactly one selected plan (93-104). Qualification: per issuer, collect granted plans whose schema has a method with the issuer's requirement-identity text (117-134); prefer plans whose trait-name text and package equal the issuer's boundary path/package when `issuer.is_product_owner` (135-152); two or more matches is an error, zero or one is fine (158-165). Import applications require exactly one declaration provenance per plan (282-291).
4. Same job elsewhere. none.
5. Callers (grepped). `bind_selected_provider_plan_facts`: 1 (`selected-dispatch/provider_binding.rs:155`). `validate_qualification_grants`: 1 (line 76, plus re-export `lib.rs:42`).
6. Limits. Header 13-17: when the owner slot is not granted "the unscoped matches keep their fail-closed behavior".
7. Fail-open. `[] | [_] => {}` (159): an admitted qualification requirement matching zero granted plans passes. Open: the header only discusses ambiguity.

## provider-selection/src/binding/installation_reach.rs (121 lines)

1. Purpose. Resolves the installation reach closure: rows with dependencies become concrete once their children are resolved, iterating to a fixed point.
2. Shape. `InstallationReachRow` (13), `resolve_installation_reach` (23), `completed_row` (87), `plan_scoped_resolution` (99).
3. Decides. A dependency resolves from the owning plan's row first, else from any plan only if the requirement has one publisher (`publishers` BTreeMap<String, BTreeSet<u64>>, 29-30, 112-117). Loop stops when no progress (73-75); leftovers error with "conservative bound, not a resolved row" (80-83).
4. Keys. requirement-identity string and plan report fingerprint (`u64`).
5. Callers. 1 (`binding.rs:217`).
6. Limits. Quoted: "its checked reach is a conservative bound, not a resolved row" (81).
7. Fail-open. none seen; a dependency cycle ends in the leftover error.

## provider-selection/src/binding/provider_instances.rs (212 lines)

1. Purpose. Attaches exact checked instances to whole selected plans: decides cardinality, template-row agreement and complete closed-requirement coverage.
2. Shape. row/generic/plan declaration structs (14-34), `project_provider_instances` (36), `append_instance` (196).
3. Decides. Row binding picks `selected_machine`: `CheckedAdapter` machine identity, `CompilerIntrinsic|Syscall` the requirement identity, all others `continue` (64-72). One candidate per application else error (98-108). Receiving application must match the plan schema (91-97). At most one generic template per plan (122-131); template row must equal the template text (144-150); every `required_instances` entry must be served by exactly one row (172-185).
4. Same job elsewhere. Source-side preparation in `provider-planning/provider_instances.rs`.
5. Callers. `project_provider_instances`: `provider-planning/provider_instances.rs:75`.
6. Fail-open. `[] => continue` (99) and `candidates.is_empty() { continue }` (81): an application with zero candidates is skipped. Open (no comment). Bindings other than the three above are skipped (71).

## provider-selection/src/bodies.rs (197 lines)

1. Purpose. Selects the checked body (from Psi checked-operator candidates or Terminal candidates) for a selected checked-adapter row, and joins external numeric execution meaning.
2. Shape. `select_checked_body` (31), `CheckedBodySelection` (67), `select_terminal_checked_body` (112), `selected_external_execution` (144), `selected_execution_for_identity` (167).
3. Decides. `select_checked_body` requires a one-row plan (35-40) and exactly one candidate matching product flags, requirement text, requirement package, callable, package, target, provider path, provider package, application and lifetime partition (42-57). `CheckedBodySelection::matches` accepts any candidate target when `target.is_none_or(...)` (108). Numeric: if no candidate is found, `reconcile_numeric_meaning(None, retained)` and return `retained` (184-187).
4. Same job elsewhere. none.
5. Callers (grepped). `select_checked_body`: `boundary.rs:264`, `provider-planning/.../source_provider_selections.rs:178`. `selected_execution_for_identity`: `boundary.rs:284`, here.
6. Fail-open. `target.is_none_or(...)` (108): a candidate with no target matches any selection target (portable body). `selected_execution_for_identity` returns `Ok(retained)` when no numeric provider is found (184-187): traced to `reconcile_numeric_meaning` (not read) which decides whether `retained` without a candidate is acceptable; open.

## provider-selection/src/bodies/callback_roots.rs (186 lines)

1. Purpose. Selects callback roots from prepared occurrence evidence, classifying a registration as foreign (import, syscall, intrinsic, table dispatch) or checked-source.
2. Shape. `SelectedCallbackRow` (18), `CallbackProviderDeclarations` (24; `prepare` 30, `unused` 64, `prepared` 71, `selected_row` 81), `select_callback_declarations` (100), `registration_is_foreign` (166).
3. Decides. Foreign iff the registration is `External` and the selected row's binding is not `CheckedAdapter` (177-185); a checked adapter must supply a checked body (179-181). No selected row -> not foreign (174-176). A use without checked entry errors if foreign and boundary-satisfied (136-139) or if a calling plan exists (140-145); with an entry exactly one calling-plan match is required (148-154).
4. Callers. `select_callback_declarations`: `selected-dispatch/provider_binding.rs:194`.
5. Fail-open. `let Some(provider) = provider else { return Ok(false) }` (174-176): an `External` registration whose requirement has no selected row is treated as not foreign, hence not made a root. Open (no comment).

## provider-selection/src/bodies/target_bodies.rs (237 lines)

1. Purpose. Computes which direct-call machines are absent for a target (propagated to callers by fixed point) and rejects calls reached from the entry that land on inert sibling bodies.
2. Shape. `DirectTargetBodyAbsences` (14; `derive` 20, `contains` 56), `reject_reached_inert_calls` (65), `family_body` (129), `explicit_body` (144), `realizes_provider_body` (160), `provider_body` (178), `unrealized_call_message` (191).
3. Decides. Absence: machine does not realize the target (25), or a call whose callee has no provider body (33-36); propagated through `callers` (46-52). Reach starts at the entry's explicit body (76-80) and follows direct callees then selected authored realizations (89-117); a callee with no explicit body for the target is refused (99-111). Target compare is string equality of target names (156, 175).
4. Shapes. Message branches on one declared target vs several (218-235).
5. Same job elsewhere. `realizes_provider_body` vs `provider-planning/target_machines.rs:187 realizes_machine`; `family_body`/`provider_body` vs `target_machines.rs:176-181`.
6. Callers (grepped). `DirectTargetBodyAbsences::derive`: 1 (`plan_derivation.rs:186`). `reject_reached_inert_calls`: 1 (`target_machines.rs:125`).
7. Limits. Header 1-4: "Absence is negative evidence, not a realizability certificate"; "neither operation walks source or infers implicit operator, cleanup, or dynamic edges."
8. Fail-open. (a) `call.machine == None` calls are skipped in `derive` (30-32) and contribute no absence. (b) In `reject_reached_inert_calls` a provider-row realization with no graph position is dropped by `filter_map` (97) and never checked. (c) `entry == None` or an entry with no explicit body yields an empty frontier and `Ok(())` (76-80, 119-121). Stated only for implicit/dynamic edges (header 3-4); the others are open.

## provider-selection/src/boundary.rs (357 lines)

1. Purpose. Admits exact executable boundary applications from the Terminal catalog (`select_boundary_applications`) and admits selected declaration policy under canonical Product demand (`BoundaryProviderAdmission`) in three phases.
2. Shape. `CompilerIntrinsicPlanProvenance` (26), `select_boundary_applications` (33), `BoundaryProviderAdmission` (151; `admit` 159, `admit_requirement` 210, `admit_operator` 239, `require_numeric` 276), `OperatorEvidence` (302), `BoundaryUse` (307), `selected_plan` (312), `requirement_binds` (352).
3. Decides. Matching selected row for a demand: trait-name text == `requirement.overload()`, package equal, row requirement identity text == overload (69-75), exactly one (76-95). Binding kinds: `CompilerIntrinsic` -> `selected_external_execution` (96-113), `CheckedAdapter` -> `select_terminal_checked_body` -> `SpecializedCheckedBody` or `NongenericCheckedBody` by `body.specialization` (114-131), any other -> error (132-136). Admission: operators accept `CheckedAdapter` and `CompilerIntrinsic`; direct calls accept `CheckedAdapter|Import|Syscall|CompilerIntrinsic` (225-236 vs 251-272).
4. Shapes. Selected plan must have exactly one row (343-348). `GenericTemplate` application evidence returns `Ok(())` without checking (255-257).
5. Same job elsewhere. `requirement_binds` equals `service_schema::schema_binds_exact_boundary_*` text+package pair in provider-planning.
6. Callers (grepped). `select_boundary_applications`: 1 (`pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:35`).
7. Limits. Header 6-9: "Source declaration eligibility is a separate scope because unused generic declarations need no portable executable body".
8. Fail-open. (a) `selected_plan` returns `Ok(None)` when no candidate in `candidates` binds the requirement (318-323), and `admit` `continue`s when no review declaration binds (165-172): the check is skipped for uses with no demand plan. Partly stated by the header (6-9). (b) `GenericTemplate` evidence skipped (255-257); stated: generic templates need no portable body.

## provider-selection/src/choice.rs (251 lines)

1. Purpose. Whole-provider choice and admission over prepared declaration records: `choose` picks plans via the language-semantics chooser, `admit` checks operator-family completeness, independent component closure, declaration correspondence and builds `SelectedProviderPlanFacts`.
2. Shape. `is_candidate_demanded` (33), `choose` (48), `admit` (80), `same_family` (178), `check_operator_families` (190).
3. Decides. `choose` marks candidates with `native_build` as `NativeBuild` regardless of the chooser's reason (67-71). `admit` sorts by `selection_order` (90); per plan: composition mode (96-102); independent edges join through `IndependentComponentJoin::realize` (103-115); `UniqueCoveringCandidate` is an error ("no authored target selection or declared default", 119-125); `NativeBuild` skips correspondence (126-128); otherwise each declaration's schema and provider must correspond to the candidate's (140-151). Operator family: every coordinate must be realized by exactly one selected plan (214-247).
4. Shapes. Order of checks in header: families, independent, correspondence, facts.
5. Callers (grepped). `is_candidate_demanded`: 0 call sites outside its definition and re-export (`lib.rs:34`); the name appears only in a source-text guard `compiler/src/product_boundary.rs:712`. `choose`: `provider_planning.rs:177`, `publication.rs:365`. `admit`: `provider_planning.rs:144`, `publication.rs:368`.
6. Fail-open. `NativeBuild => continue` (126-128): no declaration correspondence is checked for native-build selections. Traced: `native_build` is the `all(native)` flag set at `provider_planning.rs:347-351` and `provider_publication.rs:157-161`; true also for zero-row plans (see provider_planning.rs note).

## provider-selection/src/choice/declarations.rs (199 lines)

1. Purpose. Types for canonical selection: `ProviderIdentity`, `ProviderFamily`, `ProviderSubject`, `ProviderDeclaration`, `SelectionProvenance`, `CandidateDeclarations`, `ProviderCandidate`, `SelectedProvider`.
2. Shape. `same_declaration` (41: `Ok == Ok` only), `ProviderSubject::{selects_plan, selects_schema, same_declaration_as, owner_errors}` (49-119), `SelectionProvenance::{declarations, composition_mode}` (139-177).
3. Decides. `selects_schema`: trait/requirement subjects need equal declaration and application (`application_identity.is_none_or(...)` 80); operator family matches coordinate declaration plus requirement-identity text (82-85). `same_declaration` returns false if either side is `Err` (45). Composition: target defaults may not request independent (150-157); build overrides must agree (165-173).
4. Limits. Comment 12: "missing portable ownership is a deferred refusal, never an identity".
5. Fail-open. `identity.application_identity.is_none_or(..)` (80): a selection with no application matches every application of the schema. By design for non-applied selections; not stated at site.

## provider-selection/src/choice/independent_components.rs (133 lines)

1. Purpose. Joins `Independent` selections to verified component descriptions: every independent plan realized by exactly one component, every component realizes at least one plan.
2. Shape. `IndependentComponentJoin` (23; `realize` 37, `finish` 111).
3. Decides. Delegates to `VerifiedComponent::realizes_selected_plan(plan, selecting_declaration)` (52); unresolved installation rows reported separately (54-56, 68-90).
4. Callers. `choice.rs:92,106,154`.
5. Fail-open. none seen (no components -> error 42-47).

## provider-selection/src/choice/selection.rs (290 lines)

1. Purpose. Adapts plans and declarations to `language_semantics::declaration_selection::provider_choices::choose` and maps each error to a diagnostic text.
2. Shape. `ProviderSelectionKey` alias (11), `provider_slot_key` (14), `selected_subject_keys` (24), `selected_provider_key` (54), `provider_plan_key` (64), `NativeCandidate` (78), `NativeDeclaration` (138), `select_provider_plan_indices` (167).
3. Decides. Slot key `(trait_package_identity, trait-name text, root_application_identity)` (14-22); provider key `(provider_type_package, provider_type path string, application_identity)` (64-70). Candidates expose `covers_requirements()` (plan covers schema) and receiver relations (116-135). Authority tiers are `Override` -> "build" and `Default` -> "target package" (200-203).
4. Single-policy claim. Header 1: "One whole-provider choice policy for invocation and canonical declarations." Grepped `select_provider_plan_indices|provider_choices::choose`: one caller of the adapter (`choice.rs:55`) and one call to the chooser (190).
5. Fail-open. none seen (errors enumerated 204-264).

## provider-selection/src/lib.rs (53 lines)

Purpose. Crate root with docs ("Start at publication.rs...") and re-exports. No decisions. Fail-open none.

## provider-selection/src/publication.rs (608 lines)

1. Purpose. Selects canonical Build companions against the emitted Terminal demand catalog: grows provider application demand to a fixed point, chooses and admits once, and produces `TerminalTargetProviderProposal`s per target.
2. Shape. `CanonicalProviderDeclaration` (34), `PreparedProviderCandidate` (52; `accepts_choice` 63), `PreparedTargetChoices` (102; `map_owner` 143, `without_correspondence` 182, `close_application_roots` 307, `select` 318, `extend_application_demand` 412), `ProviderApplicationDemand::{Product, Invocation}` (112), `SelectedTargetProviders` (117), `TargetProviderChoice` (126), `SelectedTargetProviderPublication` (135), `PreparedTargetProviderPublication::{new, publish}` (214-304), `describe_selection` (548), `close_application_roots` (568).
3. Decides. `select` loops until the described selection equals the previous one (326-409); `demanded` only grows (536 `|=`), so termination is by monotonic growth (inferred; no explicit bound). A candidate becomes demanded if its schema owner text is in `invoked_owners` (523-525) or it is explicitly selected (and, for Invocation, an inherited owner is invoked 526-535). Roots close by `(owner, callable)` text equality to a unique proof declaration (579-597). `publish` rejects a retained choice from another Terminal artifact, target, root set or assumed bindings (243-263).
4. Cases. `close_application_roots` accepts only `CanonicalProviderDeclaration::Machine` roots (576-578).
5. Same job elsewhere. `PreparedTargetChoices::select` re-runs the `choose`/`admit` chain used by `provider_planning.rs:144,177`.
6. Callers. `ProviderApplicationDemand::Invocation` has 1 construction site: `provider-planning/src/calling_policy_plans/selected_providers.rs:88` (grepped).
7. Limits. Header lists the fixed-point rules (12-28).
8. Fail-open. In `extend_application_demand`, operations with no matching demand record are skipped (503-506 `Err(_) => continue`), and machines with no declaration are skipped (494-497). Inferred: these are non-boundary operations/machines. Open (no comment).

---------------------------------------------------------------------
## selected-dispatch/src/compiler_intrinsic.rs (140 lines)

1. Purpose. Joins prepared declaration facts to target and accepted provider custody to derive a selected compiler-intrinsic execution identity for a row.
2. Shape. `SelectedCompilerIntrinsicExecutionIdentity` Closed/Unsupported (22), `derive_selected_compiler_intrinsic_execution_identity_for_row` (27), `row_execution_identity` (70), `numeric_execution` (108), `settle_declaration` (127).
3. Decides. Non-`CompilerIntrinsic` rows return `None` (83-85). Numeric: `BoundaryOperator` schemas, or `BoundaryRequirement` schemas with a custody requirement schema (116-118), use `numeric.execution(..)`; otherwise `IntrinsicDeclaration::prepare` then `settle(plan, target, console, process_exit)` (96-105, 135). `settle` returning `None` -> `Unsupported` (136-137).
4. Callers (grepped). 1 external (`review-evidence/.../intrinsics.rs:32`).
5. Limits. Header 1-7: preparation and a closed catalog identity "supplies [no] host execution permission".
6. Fail-open. `Unsupported` is a returned value, not an error (136-138); `intrinsic_review.rs:262-265` treats it as "accumulate role match, row None". Open (whether an Unsupported intrinsic is rejected later is not visible here; header 12-14 of `intrinsic_review.rs`: "package review rederives and rejects unsupported children").

## selected-dispatch/src/intrinsic_review.rs (434 lines)

1. Purpose. Resolves accepted semantic service bindings, retains row-aligned compiler-intrinsic review identities after checked provider execution is selected, and produces direct boundary leaves.
2. Shape. `ResolvedAcceptedSemanticBinding` (33), `resolve_accepted_service_binding` (52), `retain_selected_compiler_intrinsic_review_identities` (93), `produce_direct_boundary_leaves` (324), `validate_terminal_authority_permissions` (409).
3. Decides. Accepted roles allowed: Filesystem/Time host service and six `*ProgramEntry` roles (56-66), only requirement-only bindings (`selected_provider_plan_digest().is_none()`); resolved service schema digest and package must match (77-85). Console/ProcessExit roles consume at most one match each (276-313: zero or many matches is an error). Hosted exit row identity `HostedExitProcessI32` triggers `accumulate` (256-258). Per-plan unique candidate by `matches_selection` (130-142). Placed-catalog custody check (151-170). `validate_terminal_authority_permissions` requires each permission's requirement text to match exactly one schema method (418-431).
4. Shapes. Fixed role list at 56-65. Two exit roles hard-coded (276-279; `AcceptedSemanticBindingRole::ConsoleExitProcessI32`, `ProcessExitExitProcessI32` at 391-392).
5. Same job elsewhere. Role consumption also at `provider_binding.rs:247-257` (Filesystem/Time/entry).
6. Callers. Both pub(crate) fns have 1 caller each in `provider_binding.rs` (219, 231).
7. Limits. Header: "Non-intrinsic and unsupported compiler-intrinsic rows retain None so package review rederives and rejects unsupported children until a closed identity exists."
8. Fail-open. (a) `accepted` closure (353-365) returns `Ok(None)` if no binding of that role is resolved or no selected provider has the digest, and `settle` proceeds with `None` bindings (389-394). Open. (b) `rows.push((!identity.is_numeric()).then_some(identity))` (260) keeps numeric identities out of retained rows by design.

## selected-dispatch/src/lib.rs (43 lines)
Purpose: crate root and re-exports. No decisions.

## selected-dispatch/src/numeric_provider.rs (97 lines)

1. Purpose. Joins a selected plan row to Psi's external numeric declarations, requiring candidate and custody copies to agree.
2. Shape. `NumericProviderSelection` (10), `has_requirement_schema` (17), `execution` (20).
3. Decides. Requirement nominal must equal in candidate and custody (28-34); realization has a custody nominal (35-38); `selected_numeric_provider` runs on both and the tuples (meaning, realization identity text, owner text, requirement schema texts) must be equal and the realization and provider owner must equal the selected ones (72-87).
4. Callers. `compiler_intrinsic.rs:120`, `intrinsic_review.rs:232-243,374-386`.
5. Fail-open. `provider: None` skips the owner comparison only if `candidate.2 != provider` is compared as `Option` (82): `candidate.2` (Option<&str>) vs `provider` (Option): with `provider == None` and `candidate.2 == None` passes; with `provider == None` and `candidate.2 == Some` fails. Traced: closed in that direction, open in the converse (a selection with no recorded provider is accepted when the candidate also has none).

## selected-dispatch/src/provider_binding.rs (275 lines)

1. Purpose. Prepares `SelectedExecution`: captures declarations while source exists, then binds a target's selected implementations without reopening source.
2. Shape. `PreparedBindingDeclarations` (22; `capture` 34), `ProviderBindingRequest` (82), `prepare_selected_execution` (107).
3. Decides. Order (107-274): owner pointer checks (110-118), target equality (119-125), package-aware non-provider grant rejection (131-138), grant/boundary binding (155-173), callback roots (187-208), service declarations (209), exit-role retention (219-230), direct leaves (231-240), then Filesystem, Time and entry roles resolved (247-257).
4. Same job elsewhere. none.
5. Callers. `prepare_selected_execution`: compiler (not read).
6. Fail-open. `accepted_binding(role)` returns `None` when package inputs are absent (214-218) and the role is simply skipped (252); a missing accepted Filesystem/Time/entry binding is not an error here. Open (consumers not read).

## selected-dispatch/src/selected_dispatch.rs (233 lines)

1. Purpose. Settles floating-point `Match` equality executions for selected providers and exposes `derive_selected_primitive_float_binary_execution`.
2. Shape. `settle_selected_comparison_execution` (44), `selected_float_comparison_executions` (55), `replace_executions` (160), `derive_selected_primitive_float_binary_execution` (171), `selected_numeric_execution` (186).
3. Decides. Only `CheckedOperatorOccurrence::MatchEquality` uses with a selected float comparison (68-80); only exactly `Equal` with `PrimitiveFloatBinary{Equal, F32|F64}` is accepted (128-143). No provider in Build scope is an error, in Product scope `continue` (96-101); no admitted execution likewise (120-127). One provider per requirement with one row and authored realization in context (102-112).
4. Cases. Hard-coded: only `F32`/`F64` and only `Equal` (128-143).
5. Callers (grepped). `settle_selected_comparison_execution`: 1 (`build-settings/src/admission/checking.rs:206`). `selected_float_comparison_executions`: internal (50) plus test support `compiler/tests/support/checked_interpreter_dispatch/mod.rs:47`. `derive_selected_primitive_float_binary_execution`: `selected_constants.rs:96` and line 114.
6. Limits. Header: "unlike the product's provisional route, every Build demand must have an executable selected child."
7. Fail-open. Product scope `continue` when no provider (96-101) or no float execution (120-127): stated (header 9-12).

## selected-dispatch/src/selected_dispatch/operator_adapter.rs (217 lines)

1. Purpose. Shared joins for checked boundary-operator realizations: resolve the checked adapter body, the operator definition, the selected plan for a use, the machine contract and the overload identity.
2. Shape. `resolve_checked_adapter_for_operator` (27), `exact_operator_definition` (129), `selected_use_plan` (149, wrapper over provider-planning), `resolve_exact_selected_plan` (165), `exact_realization_contract` (179), `requirement_overload_identity` (205).
3. Decides. Plan must have one method named `"realize"` and one row binding the overload identity and package (41-72, hard-coded `method.name != "realize"` 62); application must be exactly one at the expression (100-114); contract must be exactly one and its commitment non-zero (184-203).
4. Same job elsewhere. `provider-planning/.../adapter_conformance.rs:13-105` (see that entry). `selected_use_plan` wrapper duplicates `provider_planning::selected_use_plan` with scope fixed to Product (155-161).
5. Callers. All pub(super); used by the two realization modules.
6. Fail-open. Returns `Ok(None)` when the row is not `CheckedAdapter` (74-76); the callers treat that as "no realization" (application_realization.rs:224-228), but the specialized one errors (specialized_application_realization.rs:99-105). Asymmetry noted.

## selected-dispatch/src/selected_dispatch/operator_adapter/application_realization.rs (288 lines)

1. Purpose. For canonical-empty (non-generic) boundary operator applications, rejoins selected scalar calls to the exact nongeneric checked body and records provider plan commitment, realization machine/state and contract commitment (label "D29" in header).
2. Shape. `CheckedOperatorAuthoredUseKind` Named/FixedToken (37), `CheckedNongenericOperatorApplicationRealization` (43), `derive_checked_nongeneric_operator_application_realizations` (56), `derive_checked_expressions` (79), `derive_checked_expression_operator_application_realization` (107), `has_exact_authored_selection` (263).
3. Decides. Skips applications with arguments (87); `MatchEquality` sites return `None` (112-117); top-level requirement machines return `None` (125-131); authored use must be exactly one (named use by expression/origin/symbol, or fixed-token resolved boundary use with matching spelling 139-183) and exact (184-188); requires an authored declaration selection of the matching kind (189-196, 263-288); `None` if no selected plan (197-204) or the plan is not a single `CheckedAdapter` row (213-222). Duplicate site derivations must be identical (63-74).
4. Cases. Header 16-22: implicit `MatchEquality` comparisons and direct calls to top-level requirement machines return `None`; "compiler-intrinsic or admitted external roles return None".
5. Same job elsewhere. "Authored uses" collection (139-177) is repeated in `specialized_application_realization.rs:181-230`.
6. Fail-open. `Ok(None)` for no authored selection (195), no selected plan (203), non-checked-adapter row (221), `resolve_checked_adapter_for_operator` returning `None` (227): a use that fails any of these produces no realization row and no error here. Stated by header lines 20-22 for the plan and role cases; the missing authored-selection case (189-196) is open.

## selected-dispatch/src/selected_dispatch/operator_adapter/specialized_application_realization.rs (242 lines)

1. Purpose. For non-empty (generic) boundary operator applications, rejoins checked use, selected plan and authoritative machine specialization to one exact specialized body.
2. Shape. `CheckedSpecializedOperatorApplicationRealization` (26), `derive_checked_specialized_operator_application_realizations` (42), `derive_one` (65), `exact_authored_uses` (181), `applications_match` (232).
3. Decides. Only `Expression` use sites (70-76); operator must be boundary and generic (78-82); exactly one authored use (86-91); selected plan must exist (92-98, error); realization must be a `CheckedAdapter` (99-105, error); exactly one specialization for the instance, one retained realization row for the requirement, and `checked_operator_application_matches_realization` (107-143); entry state equals the realization state (154-158).
4. Same job elsewhere. `exact_authored_uses` repeats `application_realization.rs:139-177` without the exactness boolean.
5. Callers. Through `selected_dispatch.rs:39-42` and `lib.rs`; not grepped further (only definitions seen).
6. Fail-open. none seen (all missing parts return errors).

## selected-dispatch/src/selected_execution.rs (118 lines)

1. Purpose. `SelectedExecution` record: all selected implementation data for one target, with accessors for native target, entry service requirements, resolved semantic bindings, terminal authority permissions, and `candidate_service_binding`.
2. Shape. struct (25-42), `selected_native_target` (45), `program_entry_service_requirements` (50), `plans` (54), `resolved_semantic_binding` (58), `accepted_semantic_bindings` (67), `terminal_authority_permissions` (75), `candidate_service_binding` (100).
3. Decides. `resolved_semantic_binding` returns the first resolved binding with a role (62-64). `terminal_authority_permissions` re-interns identities with `unwrap_or_default` (88-91).
4. Fail-open. `identities.text(..).unwrap_or_default()` (88-91) turns a missing permission requirement identity into the empty string re-interned in the output table; traced: upstream `validate_terminal_authority_permissions` (`intrinsic_review.rs:409-434`) already requires each permission to match one method, so an empty text would fail that check first for bindings that went through resolution (traced); open for any path that skips it.

---------------------------------------------------------------------
## trust-model/src/accepted_machine_grants.rs (147 lines)

1. Purpose. Resolves a root grant string to an accepted machine and rejects individual accepted-machine grants in package-aware compilations.
2. Shape. `NonProviderTrustGrant` (13), `reject_package_non_provider_grants` (23), `reject_package_grants_with_providers` (38), `resolve_non_provider_trust_grant` (63), `validate_grant_subject` (101), `ambiguous_grant` (115), `accepted_machine` (127).
3. Decides. By source spelling of the grant: exact name equality with `machine.name` (78), else, when the grant contains no `::`, leaf-name equality `name.rsplit("::").next() == Some(grant)` (85-95). Exactly one match each; otherwise error (96-99).
4. Cases. Only `AcceptedMachine` exists in `NonProviderTrustGrant` (13-15). Error text: "domain and arbitrary-string trust grants are unsupported" (97).
5. Callers (grepped). `reject_package_non_provider_grants`: `selected-dispatch/provider_binding.rs:132`.
6. Fail-open. Grants resolving to a selected provider selector are skipped in the package-aware check (44-49). `accepted_machine` requires exactly one symbol match (136-145). None seen.

## trust-model/src/admission_settlement.rs (155 lines)

1. Purpose. Reconstructs the `TrustAdmission` obligations that a root's grants establish.
2. Shape. `PreparedTrustIdentity` (16), `PreparedTrustObligation` (28), `reconstruct_trust_obligations` (33).
3. Decides. Provider grants take precedence over machine grants (51-66). Machine: template error -> refusal (74-88), `Some(template)` -> `MachineTemplate`, `None` -> `AcceptedMachine(contract)` with contract errors deferred until constructing the admission (93, 124-138). Duplicates collapse by commitment string (56, 101); a duplicate commitment in the final map errors (147-152). Output ordered by `BTreeMap<String, TrustAdmission>` (109, 154).
4. Callers (grepped). 1 production (`omega/compilation/products/src/checked_compilation.rs:166`) plus tests.
5. Fail-open. With `package_aware == false`, machine grants are accepted (47-49 only runs the package check when true). By design (lib.rs:15-17).

## trust-model/src/lib.rs (51 lines)
Purpose: crate docs and re-exports; `AuthoredComponentAssumptionAcceptance { digest, selecting_machine, source_span }` (47). Users (grepped): `build-settings/src/lib.rs:123` (re-export), `products/src/evaluated_build.rs:14,43`, `products/src/package_build/retention_identity.rs:423`. Fail-open none.

## trust-model/src/tests.rs (197 lines)
Test module: eight `#[test]` functions over `reconstruct_trust_obligations` and `TrustAdmission` (exact-vs-leaf resolution 45, deferred unused errors 67, template cardinalities 81, contract cardinalities 100, package policy before template evidence 127, duplicate symbol/invalid symbol/unknown grant 139, digest identity 157, selector failure precedence 191).

## trust-model/src/trust_admission.rs (170 lines)

1. Purpose. `TrustAdmission` and its domain-separated digest (`omega.trust-admission.v1\0`, subjects `provider-plan`, `machine-template`, `machine-contract`).
2. Shape. `TrustAdmissionDigest` (14; rejects all-zero 18), `TrustAdmissionSubject` (39), `trust_admission_digest` (55), `TrustAdmission` (68) with constructors `for_provider_plan` (100), `for_machine_template` (113), `for_machine_contract` (126).
3. Decides. Equality and ordering use `(commitment, digest)` only; `report_identity` is excluded (152-169). Commitment must be non-empty single-line (75-84). Note `from_digest` all-zero check (18) is not applied in `trust_admission_digest` (64 constructs directly).
4. Fail-open. none seen.

---------------------------------------------------------------------
## omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/access_references.rs (120 lines)

1. Purpose. Validates retained access-reference backings against their placed plan, and derives a borrowed-view formation from a boundary lifecycle declaration.
2. Shape. `validate_backing` (16), `borrowed_view_formation` (63).
3. Decides. Source type, schema and carrier must equal the plan's (23-28); `OrdinaryRam` compares to `access_ram_plan_correspondence` (30-37); `Resident` compares the placement commitment, owner declaration, owner field roles and requires `granted != resident` roles (38-49); `BorrowedExtent` compares the commitment and owner fields (50-58). Formation returns `None` unless the declaration's placement operation is `View`, the result structural type matches, the plan has `vacant`/`resident` unset, a view reference slot exists, and the placed view's schema/policy/placement equal the plan's (71-107).
4. Callers (grepped). `validate_backing`: 6 call sites across `unit_validation.rs:7745`, `selection_inputs.rs:4188`, `legalization_reference_custody.rs:6167,7530`, `references.rs:736,2113`. `borrowed_view_formation`: 3 (`control_flow_storage.rs:135`, `legalization_reference_custody.rs:6034`, `descriptor_results.rs:101`). The Psi-side `validate_backing` at `content_transformations.rs:1401` is a different function with the same name (grepped).
5. Fail-open. `borrowed_view_formation` returns `None` on any mismatch (71-107), i.e. "no formation"; callers decide. Open (callers not read).

---------------------------------------------------------------------
## Coverage

- Read in full: all 54 files, lines 1..last for each; total 13,640 lines (sum of the line-count column; I summed 7,550 + 3,163 + 2,087 + 720 + 120). Skimmed: 0. Not read: 0. (Large files read in one Read call each: completion.rs 767, plan_derivation.rs 685, publication.rs 608.)
- Out-of-unit sites were grepped only, never read: `native_bindings/plans.rs`, `intrinsic_declarations.rs`, `program_entry.rs`, `calling_policy_plans/selected_providers.rs`, `compiler/src/product_boundary.rs` (lines 695-735 read), `compiler/src/execution_settlement.rs` (lines 349-365 read via grep context).

## Five things a maintainer most needs to know

1. `provenance_replay.rs:8` says derivation is the single producer of `ProviderPlan`/`ProviderPlanProvenance`, but plans are also built in `named_provider_plans.rs:72,90` and, outside this unit, `native_bindings/plans.rs:165,178` (grepped); the three-way `derived_plan_position` / literal construction is the same job spelled twice inside the folder (`plan_derivation.rs:579-602`, `named_provider_plans.rs:72-103`).
2. Several judgments key on source spelling: the four hosted catalog names `ConsoleNativeProvider::exit_process|write_byte|read_byte`, `ProcessExitNativeProvider::exit_process` (`provenance_replay.rs:234-260`) and the trait name `"Console"` (`semantic_roles.rs:51`), the `"compiler-catalog::placed-scalar.v1::"` prefix (`sealed_accessors.rs:119`), the operator method name `"realize"` (`operator_adapter.rs:62`, `plan_derivation.rs:416`), and root-grant name/leaf-name matching (`accepted_machine_grants.rs:78,88`). Plan identities elsewhere are (package, trait-name text, application) tuples (`selection.rs:14-22`, `boundary.rs:352-357`, `service_schema/mod.rs:85-113`) built in at least three places.
3. "Does this machine realize this target" is implemented twice: over typed trees (`target_machines.rs:187`) and over `DirectCallDeclarations` (`target_bodies.rs:160`), each with its own family-member lookup; both treat "no target context" as realizing every target and an empty/None target as the host (`target_machines.rs:179,200`, `plan_derivation.rs:190`).
4. Checks that are skipped when the anchor is missing: no inert-call check without a selected entry (`target_bodies.rs:76-80`, `execution_settlement.rs:360-362`); no completion review for non-`Authored` rows (`completion.rs:30-33`); boundary admission skipped when no demand plan binds (`boundary.rs:318-323`); qualification requirements with zero granted plans pass (`binding.rs:159`); native-build selections skip declaration correspondence (`choice.rs:126-128`) and `native_build` is true for a zero-row plan (`provider_planning.rs:347-351`).
5. Dead or test-only public surface (grepped): `is_candidate_demanded` has 0 callers (only a source-text ban in `compiler/src/product_boundary.rs:712`); `service_schema::from_typed_operator` 0 callers; `provider_trait_requirement_matches` and `service_schema::is_product_declaration` are used only by test support; most `pub` entry points have exactly one caller (discover_explicit_entry_binding, retain_selected_provider_declarations, select_derived_provider_plans, selected_provider_plan_facts, selected_constant_target_bodies, reject_reached_inert_sibling_calls, verify_selected_compositions_are_realized, settle_selected_comparison_execution, provider_selection_roots, reconstruct_trust_obligations). Independent-component selection reaches `selected_provider_plan_facts_with_independent_components` only through the wrapper that passes empty slices (`provider_planning.rs:103`).

## What I am unsure about

- Whether `native_bindings/plans.rs` (not read) can build a zero-row plan, which would make the `native_build` flag true and skip correspondence checks.
- What `reconcile_numeric_meaning(None, retained)` (`bodies.rs:185`), `IntrinsicDeclaration::settle` (`provider-planning/src/intrinsic_declarations.rs`) and `checked_operator_application_matches_realization` do; several "open" fail-open items depend on them.
- Whether the `Ok(None)`/`continue` skips in `application_realization.rs` and `provider_rows.rs:250-255` are later reported by another stage (callers not read).
- The termination argument for `PreparedTargetChoices::select` (`publication.rs:326`) is inferred from the monotonic `demanded` vector, not proved.
- `provider_choices::choose` in `language_semantics` (the actual chooser) was not read; the "single policy" claim was checked only by grepping call sites of the adapter.
