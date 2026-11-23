# Record: unit_007 (area 09-native-stages-00-03)

Path abbreviations (all relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`):
- `S1/` = `01_admitted-native-inputs-to-target-bound-program/` (files are under `S1/src/` unless noted)
- `S2/` = `02_target-bound-program-to-abstract-operations/`
- `S3/` = `03_abstract-operations-to-abstract-operations/src/abstract_optimization/`

Every file in the unit was read in full (34 files, 21,253 lines). Test counts come from the Grep count mode of `#\[test\]`
over each stage directory. "Read" means I read the cited lines; "grepped" means a Grep count I ran; "inferred" means I did not read the other site.

Cross-stage facts that apply to many files (read):
- Almost every judgment in stage 01 and stage 02 is keyed on **identity text compare**: `module.identities.text(id) == Some(string)`
  (examples: `S1/program_entry/calling_plans.rs:37-51,77`, `S1/provider_execution_inputs.rs:99,123-124`,
  `S1/target_supply/plan_realization.rs:334,351,460`). Stage 03 judgments are keyed on dense ids (`MachineId`, `BlockId`, `ValueId`, `OperationId`) and on `PsiOptimizationUnit` identity digests.
- Stage 03 comments name stages "Omega 00/01/02" (`S3/mod.rs:4,25,29`) and stage 01's reader comment says "Stage 03 owns this value"
  (`S1/target_supply/realized_supply.rs:45`) while 02's comment says "Omega 00" (`S2/selected_execution.rs:44`). The directory numbers are 01, 02, 03. Numbering is stale in comments only.

---

## Stage 01 files

### S1/program_entry/calling_plans.rs (214 lines)
1. Purpose: evaluates the entry schema's calling policy twice (semantic signature and physical signature) and builds the pair of calling applications plus the storage entry plan for the program entry.
2. Shape: `ProgramEntryCallingPlanRequest` (:19, fields `slot`, `schema`, `declaring_schema`, `semantic_requirement`, `physical_requirement`, `physical_source_digest`, `providers`, `admission`); `entry_calling_policy` (:30); `signature` (:66); `SelectedProgramEntryCallingPlans::evaluate` (:92).
3. Inputs/outputs: reads `TerminalModule.interfaces.calling_policy_requests` and `calling_abi_signature`; calls `CallingProgram::evaluate` (:133,135); returns `SelectedProgramEntryCallingPlans {semantic_calling_application, physical_calling_application, storage_entry}` (:208). Pushes two `ServiceCallingApplication` rows into the cloned `schema` (:149).
4. Decides: policy chosen by `declaration` text equal and `declaring_application` text equal to `owner` (:37-51); signature chosen by `requirement_identity` text equal (:77). Two matches is an error ("competing", :55, :81). Semantic and physical signature ids must differ (:128). A zero 32-byte source digest is refused (:164).
5. Cases by shape: exactly one method per requirement in the schema (:115 `methods.next().is_none() || methods.next().is_some()`).
6. Same job elsewhere: `S1/provider_execution_inputs.rs:263-312` also loops policy `signatures` and calls `calling.evaluate`/`native_application_with_owner`, for provider requirements instead of the entry.
7. Unused: `entry_calling_policy` grepped 4 occurrences in 3 files (def, `program_entry.rs`, `calling_plans.rs:125`, `published.rs:91`), so 3 call sites outside its definition.
8. Stated limits: "no product route yet: entry schema ... has no calling policy" (:62), "no product route yet: entry requirement" (:87).
9. Fail-open: none seen.

### S1/program_entry/published.rs (209 lines)
1. Purpose: records which entry the published program nominates for a target, then settles its calling plans against the already-chosen providers.
2. Shape: `PublishedEntryNomination` (:13, `program`, `selected`, `schema_declaration`), `from_module` (:20), `plan_published_entry_calling_conventions` (:55), `selected_method` (:145), `admit_entry_schema` (:164), `requirement_identity` (:131).
3. In/out: reads `module.program_entry_contract(target_name)`, `service_schema`, `AcceptedSemanticBinding`, `ProductDeclarationScope`; produces `SelectedCompilerProgramEntry` with calling plans and a bound physical adapter (:126-128).
4. Decides: `selected_method` matches method by `requirement_owner == owner && name == name` after `rsplit_once("::")` of the nominated string (:149-155), exactly one. `admit_entry_schema` (:175-208) accepts three triples of (custody, binding): toolchain contract with no binding and no trait package (:176-180); package custody whose accepted binding role, package, declaration path (string compare :185-186) and schema digest agree (:181-189); package custody with no binding whose trait name equals `slot.boundary_schema` text (:196-200). Everything else is refused (:204).
5. Cases by shape: the three custody/binding combinations above; comment :192-195 "admits only the package that alone supplies the target's contract source".
6. Same job elsewhere: `visible_parameters.rs:52` also calls `selected_method`.
7. Unused: `PublishedEntryNomination::program`/`source_signature` used at `S1/published_application/program_entry.rs:31,47` (read).
8. Stated limits: comment :192-195 (above).
9. Fail-open: none seen. The third case in `admit_entry_schema` accepts a package schema with no binding; stated at :192-195 and keyed on exact path text.

### S1/program_entry/visible_parameters.rs (163 lines)
1. Purpose: rebuilds the list of arguments a hosted entry receives from its slot (none, or two granted `Extent` parameters for a storage entry).
2. Shape: `from_terminal` (:14), `extent_layout` (:134).
3. In/out: reads the entry machine, the checked declaration, `program_entry_contract`, `service_schema`, `NativeGeometry`; returns `Vec<ProgramEntryVisibleParameterSignature>`.
4. Decides: slot `visible_parameters == None` demands no parameters at all (:33-43); otherwise requires machine id equal to nominated entry (:45), declared type signature equal (:54-61), two entry claims, and each claim `path.is_empty()` with distinct inputs (:69-85); each parameter must be the whole-Extent grant (`granted.accepts`, :110-115).
5. Cases by shape: "storage entry must consume two separate granted Extent parameters" (:67), roles fixed to `Image` then `InitialStorage` (:95-98), extra arguments refused (:25-32).
6. Same job elsewhere: none seen.
7. Unused: `from_terminal` is `pub(super)`; call site is in `S1/program_entry.rs`, not read (suspected 1).
8. Stated limits: none quoted.
9. Fail-open: none seen. (`unwrap_or_default()` on identity text at :58-59 turns a missing type name into `""` before the signature compare, so a missing name can only match an empty declared signature; traced to `declared_type_signature`, which I did not read.)

### S1/provider_execution_inputs.rs (532 lines)
1. Purpose: from the Build-evaluated bindings, package permissions and the calling policies the program selected, produce the provider execution inputs (`TargetSupplyExecutionInputs`) for the boundaries actually reached.
2. Shape: `calling_policy_instances` (:14), `require_unselected_calling_policy` (:45), `plan_provider_execution_inputs` (:61), `plan_inputs_and_review` (:69), `package_authority_permissions` (:453). Tests at :487 and :504 (2).
3. In/out: reads `ProviderExecutionOrigins`, the module's `boundary_machines`, `provider_candidates`, `external_providers`, `provider_selections`, `calling_policy_requests`; calls `CallingProgram::evaluate`, `native_calling_results`, `package_review::prepare`, `direct_compiler_primitive_leaves`; returns `TargetSupplyExecutionInputs` (:439-450).
4. Decides: boundary requirement text is the key (`requirements` is a sorted `Vec<&str>` queried with `binary_search`, :136-137, :244, :275). A Build binding joins a reached boundary when its `requirement_identity` string equals the boundary text (:147-153); two matches refuse (:155-162). An external provider whose text matches a reached requirement must be `CompilerIntrinsic`, else refused (:122-134). A native binding falls back to the default planner only when no selected policy declares that schema application (`require_unselected_calling_policy`, :332).
5. Cases by shape: core primitive classification loop (:414-438) accepts only `DirectBoundaryExecution::Intrinsic(Some(_))` (:416-421).
6. Same job elsewhere: policy-signature loop at :263-312 duplicates `S1/program_entry/calling_plans.rs:110-140` (suspected from reading both).
7. Unused: `plan_provider_execution_inputs` is `pub` and re-exported at `S1/lib.rs:31`; grepped across the repo `.rs/.md/.toml`: 0 callers (only its definition, the re-export, and a `_recycling/` archive file). `plan_inputs_and_review` is `pub(crate)`.
8. Stated limits: error text "needs its canonical evaluated Build binding association before physical calling inputs can be formed" (:131).
9. Fail-open: (a) :149-154 a Build binding whose requirement is not among reached boundaries is skipped with `continue`, no error; wrong input: a Build binding naming a misspelled or unreached requirement is silently dropped. Class: open. (b) `unwrap_or_default` is not used here. Others none seen.

### S1/provider_rows.rs (390 lines)
1. Purpose: builds the single-row "direct boundary" provider plan for a native leaf (external declaration, Build-bound requirement, or core primitive) and validates its calling custody.
2. Shape: `DirectBoundaryMechanism` (:7), `DirectBoundaryInput` (:13), `from_boundary` (:25), `from_checked` (:62), `finish` (:95), `direct_compiler_primitive_leaves` (:167). Test at :306 (1).
3. In/out: reads the module's `boundary_machines`, `identities`, provider schemas; mutates a clone of the original `ProviderPlan` (:105-147); returns `DirectBoundaryLeaf`.
4. Decides: plan name is a `format!("direct-boundary:{}:{}", target, callable)` (:118), a display string that other code does not parse in this unit. Primitive classification by `CheckedBoundaryExecution` variant (:205-233), with `HostedOperation::ProcessExit` the only hosted operation mapped (:211-214). Exactly one canonical schema method matching the requirement text (:235-245).
5. Cases by shape: `ProcessExit` only (:211-214, "hosted primitive has no native catalog projection"); `NumericPrimitive` through `provider_admission::direct_numeric_target` (:221-231); every other execution is skipped (`_ => continue`, :232); boundary with no `declaration` is skipped (:202-204).
6. Same job elsewhere: `S1/target_supply/plan_realization.rs:410-437,485-515` builds `ProviderPlan` rows then calls `from_checked`/`from_boundary` + `finish`.
7. Unused: `DirectBoundaryInput` grepped 7 occurrences in 3 files; `from_boundary` and `from_checked` each used once in `plan_realization.rs`.
8. Stated limits: header :1-2 "Source declaration discharge objects have no producer after the old checker connection is cut." A doc comment at :294-295 ("Target preparation and native publication consult the same canonical table...") is attached to nothing but the `#[cfg(test)]` module below (read).
9. Fail-open: :202-204 and :232 skip unmatched requested boundaries; wrong input: a requested boundary whose execution is not hosted/numeric primitive produces no leaf; the caller (`realize_target_supply`, `plan_realization.rs:598-626`) turns "no leaf" into `MissingSupplies`, so traced: not accepted silently.

### S1/provider_selection.rs (140 lines)
1. Purpose: select providers for a published program; wraps the plan with its authorization and issuing publisher.
2. Shape: `PublishedSelection<Publisher, Plans>` (:9), `select_published_providers` (:43), `plan_provider_inputs` (:88).
3. In/out: consumes `AdmittedProviderDemand` and a component map; calls `plan_unscoped_execution` or `plan_published_target_execution`; maps `MissingSupplies` into diagnostics (:121-139).
4. Decides: a `MissingSupplies` boundary id gets a human label via `labels.binary_search_by_key` (:124); otherwise prints the numeric id (:130). Keyed on id; the label is only used in the message.
5. Cases by shape: `ProviderDemandInputs::{Unscoped, Target}` (:96-117).
6. Same job elsewhere: none seen.
7. Unused: `select_published_providers` has the caller `published_application.rs:328` (read).
8. Stated limits: none.
9. Fail-open: none seen.

### S1/published_application.rs (342 lines)
1. Purpose: from the admitted demand, selects providers, settles the entry plans, checks that no callback or interrupt is reachable from the entry, builds the `TerminalNativeProposalRequest` and projects it into a `RetainedTerminalArtifact` plus permission policy.
2. Shape: `PublishedApplicationInputs` (:15), `prepare_published_application` (:25), `add_component_calling` (:154), `bind_component_export` (:174), `project_root` (:208), `NativeApplicationSettings` (:304), `bind_published_application` (:311).
3. In/out: reads `PublishedSelection`, `SynchronousCallGraph` from the entry, calling policies, package permissions; writes calling applications; returns `(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy)`.
4. Decides: callbacks/interrupt registrations are tested by `closure.machines ... operation.id == identity` (:64-85) (id keyed). Component export must be for `artifact.view().identity()` (:181). A folded artifact is re-authorized with `HostProducerTrust::accept("in-process target folding")` (:294) when not pointer-equal to the original (:290).
5. Cases by shape: "executable callback and interrupt entries need their canonical composition roots before application preparation" (:86-88) refuses any callback/interrupt reachable from the entry; `native_bindings: &[]` (:267).
6. Same job elsewhere: none seen.
7. Unused: `bind_published_application` and `bind_component_export` have driver callers (`driver/inputs/published_application.rs`, per crate map; not re-read).
8. Stated limits: :86-88 as quoted.
9. Fail-open: :294-296 stated: "effective_artifact checked the folded program against this publication." Class: stated.

### S1/published_application/program_entry.rs (74 lines)
1. Purpose: binds the nominated entry to the executable publication being prepared.
2. Shape: `PublishedNativeEntry` (:14), `for_program` (:21), `settle_calling_plans` (:59).
3. In/out: reads `execution.realization()`, `entry_nomination()`; returns `SelectedCompilerProgramEntry`.
4. Decides: nomination `program()` must equal `execution.original_program()` and slot owner must equal target profile (:31-38).
5-9: none seen. One caller (`published_application.rs:42`).

### S1/selected_execution.rs (245 lines)
1. Purpose: holds the result of provider selection (`SelectedExecution`, `SelectedProviderResult`) including realization, folded program, package review, and entry nomination.
2. Shape: `SelectedExecution` (:10), `SelectedProviderResult` (:17), `from_realization` (:44), `bind_folded_entry` (:96), `with_folded_program` (:124), `provider_proposal` (:142), `effective_artifact` (:230).
3. In/out: calls `RealizedTargetSupply::close_folded_entry`/`replay_folded_program`; `provider_proposal` copies `assumed_bindings` from the plans (:158).
4. Decides: target name equality (:49), program identity equality (`require_program`), folded `original()` equals the authorized artifact identity (:235).
5-9: `entry` is stored as `Result<_, Vec<Diagnostic>>` (:23) so an entry nomination failure is deferred until `entry_nomination()` (:179-183); not a fail-open because every consumer calls it. Fail-open: none seen.

### S1/target_supply.rs (21 lines)
Re-export module for `plan_realization`, `realized_supply`, `installation_reach`. Nothing to decide.

### S1/target_supply/installation_reach.rs (127 lines)
1. Purpose: for each selected provider that satisfies an installation requirement, computes its service reach and dependencies and feeds them to `provider_correspondence::resolve_installation_reach`.
2. Shape: `NormalizedNativeSupplies` (:7), `NormalizedNativeSupply` (:13), `resolve_installation_reach` (:19).
3. In/out: reads `module.interfaces.boundary_machines`, `services`, `root_service_reach`, a `SynchronousCallGraph`; returns `SelectedProviderPlanFacts` with resolutions attached.
4. Decides: candidate retained only if its `conformance.boundary` is in the sorted `installation_requirements` (:60-65); row requirement text must equal the boundary text (:92).
5. Cases by shape: empty requirement list is accepted only when the module's own `installation_dependencies` is also empty (:25-37).
6. Same job elsewhere: `provider-correspondence/src/installation_reach.rs` (4 grep hits, not read, suspected).
7. Unused: `NormalizedNativeSupplies` has 0 uses I saw besides re-export; `NormalizedNativeSupply` is used at `provider_execution_inputs.rs:190,206`.
8. Stated limits: none.
9. Fail-open: `realization_name: ... .unwrap_or_default()` (:101-104) makes a missing candidate name `""`. Class: open (wrong input: a candidate identity with no text yields an empty realization name in the row).

### S1/target_supply/plan_realization.rs (652 lines)
1. Purpose: builds the provider plans for each logically selected provider, builds direct-boundary leaves, checks every reached boundary is covered, and hands the lot to `RealizedTargetSupply::from_checked`.
2. Shape: `TargetSupplyExecutionInputs` (:22), `PublishedComponentSuppliers` (:34), `TargetSupplyRealizationRequest` (:42), `attach_calling` (:90), `checked_plan` (:156), `direct_leaves` (:249), `realize_target_supply` (:522); `TargetSupplyRealizationError` is a re-export of `terminal_semantics::...::ProviderSelectionError` (:154).
3. In/out: reads `ClosedLogicalProviders`, module external providers, calling applications, evaluated bindings and receipts; produces `RealizedTargetSupply`.
4. Decides: coverage is by `selected.covered.contains(boundary)` over the closed reach (:564-566), then by leaf `declaration_identity`/`callable_identity` text compare against the boundary (:607-623). External header joins to a boundary declaration through a sorted key `(machine identity, callable)` with `partition_point` (:302-313). Plain native requirements join to Build receipts by `requirement_identity` text (:447-465).
5. Cases by shape: `ExternalBindingIdentity::{CompilerIntrinsic, Syscall}` handled; any other binding is an error only if its boundary is reached (:405-407), else `continue` (:408). Comment :300-301 "A source-free external header is retained independently of a direct call."
6. Same job elsewhere: the "leaf matches boundary by declaration and callable text" predicate is written twice, `plan_realization.rs:607-623` and `S1/target_supply/realized_supply.rs:257-272` (read both; they differ: the second picks the matching boundary first).
7. Unused: `PublishedComponentSuppliers`, `realize_target_supply` are `pub`; call site of `realize_target_supply` is in `execution.rs`, not read (suspected).
8. Stated limits: :406 "reached native table leaf needs its exact evaluated storage/calling realization".
9. Fail-open: (a) `attach_calling` skips applications naming a requirement not interned in the schema (:109-115) or not a schema method (:116-123); wrong input: a stale calling application is dropped silently. Class: open. (b) :408 `_ => continue` stated by :300-301. (c) `bindings` from `request.external_bindings` with target mismatch are filtered silently (:287-291), then `from_checked` rejects foreign-target rows (`realized_supply.rs:383-389`). Class: traced.

### S1/target_supply/realized_supply.rs (614 lines)
1. Purpose: validates that selected plans, direct leaves, calling applications and evaluated bindings agree with the module, and stores the result as `SelectedProviderRealization`.
2. Shape: `ProviderPlanRealizationInput` (:15), `TargetSupplyRealizationInput` (:25), `RealizedTargetSupply` (:47), `SelectedProviderRealization` (:52), `unscoped` (:81), `no_demand` (:111), `from_checked` (:135), `resolve_installation_reach` (:440), `replay_folded_program` (:476), `close_folded_entry` (:503), `rebind_components` (:524).
3. In/out: reads module `service_schema`, `provider_selection`, `boundary_machines`, `external_providers`; produces plan roster `SelectedProviderPlanFacts` via `from_selected_plans`.
4. Decides: each plan's schema surface must equal `module.service_schema(...)` (:149); provider identity/declaration/package/application must equal the module's `provider_selection` (:177-187); composition mode determines whether a component must be present (:163-172). For each direct leaf, the boundary is found by declaration/callable text (:257-275). Folded program closure must equal `folded_constant_dependencies` and be non-empty (:495).
5. Cases by shape: leaf classes `HostedPrimitive|NumericPrimitive` (:276-290), `Trait` boundary (:291-315), other external (:316-372). `Foreign(Err(_))` executions are accepted with no check (:371).
6. Same job elsewhere: see plan_realization item 6. `S1/target_supply/realized_supply.rs:111 no_demand` and `:81 unscoped` build empty realizations.
7. Unused: `RealizedTargetSupply::unscoped`/`no_demand`/`with_folded_program` grepped together with `resolve_installation_reach`: 12 occurrences in 6 files; `with_folded_program` is called at `provider_selection.rs:73`'s wrapper only (inferred).
8. Stated limits: :81 "An unscoped check cannot choose a target's provider."
9. Fail-open: `DirectBoundaryExecution::Foreign(Err(_)) => {}` (:371) accepts a leaf whose foreign execution is an error; wrong input: a Build binding that failed to evaluate. Class: open at this site; `plan_realization.rs:389-391` creates `Err` only for a syscall with no receipt, and `execution` reaching `Foreign(Err)` for reached plain requirements is refused at `direct_leaves:463-465`. For a syscall leaf the `Err` survives (:401-402), so a reached syscall with no receipt is accepted here (traced from `direct_leaves`; I did not trace later consumers).

### S1/tests/source_provider_choices.rs (209 lines)
Test file with 4 `#[test]` functions (:88,117,187,199). It runs `plan_target_execution` on five source-published programs (`UNREACHED_DEFAULT`, `REACHED_DEFAULT`, `OVERRIDE`, `MISSING`, `FOREIGN_DEFAULT`) with `TARGET = "linux_x86_64"` (:13). It pins error strings: "logical provider choices belong to a different accepted program" (:164), "native planning changed a closed logical provider body" (:178), "no normalized chosen provider" (:205). Fixtures come from `tests/support/provider_choice_programs.rs` (not read).

---

## Stage 02 files

### S2/admit_providers.rs (370 lines)
1. Purpose: prepares the retained native providers for a lowered program and admits them against the lowered abstract plan, scoping callbacks and interrupts to the machines actually lowered.
2. Shape: `RetainedNativeProviderInputs` (:16), `PreparedRetainedNativeProviders` (:23), `lower_native_root` (:83), `prepare_native_providers` (:105), `NativeAdmissionRequest` (:146), `AdmittedNativeRealization` (:154), `admit_prepared_native_realization` (:160), `artifact_lowering_error` (:368).
3. In/out: reads `PreparedNativeExecution`, `TerminalNativeRealizationProposal`; calls `admit_native_providers` (:320), `validate_behavior_exclusions` (:322), `validate_physical_authority_closure` (:327); returns input with admitted callbacks/interrupts/assumed bindings retained.
4. Decides: a settlement stays when no boundary matches its requirement (:205 `matching.peek().is_none()`) or when a matching boundary is executed (:206); matching is `selected_row_matches_requirement(boundary text, requirement identity, report identity)` (text compare, :188-203). A native callback stays if `in_scope`: its operation is a lowered `BoundaryCall` or is not a `BoundaryCall` in the module at all (:248-251). Installed interrupt entries match callback occurrences by `placement_index` and thunk identity (:276-296).
5. Cases by shape: callbacks/interrupts are filtered only when both vectors are non-empty (:209-216).
6. Same job elsewhere: none seen.
7. Unused: four lowering/admission entry names (`lower_native_callable`, `lower_native_root`, `admit_prepared_native_realization`, `admit_executable_entry`) grepped together: 27 occurrences in 10 files (callers in `driver/mod.rs` 4, `driver/image/plan_callables.rs` 3, `driver/image/components.rs` 2, build tests 5, `tests/physical_publication/callback.rs` 3).
8. Stated limits: none quoted.
9. Fail-open: (a) :205 keeps a settlement that matches no boundary of the module. Class: open (wrong input: a provider settlement whose requirement text matches no boundary stays in the admission request). (b) :248-251 keeps callbacks whose operation is not a `BoundaryCall`. Class: open. (c) `.unwrap_or_default()` on boundary text (:197) matches an unresolved text against `""`.

### S2/entry_receiver.rs (126 lines)
1. Purpose: `admit_executable_entry` checks that the lowered entry still is the source-selected entry and settles its receiver custody.
2. Shape: `admit_executable_entry` (:20), `entry_boundary_without_receiver` (:110).
3. In/out: reads `VerifiedNativeArtifactInput`, `terminal_target_context`, `NativeProgramEntrySettlement`; returns `Option<ValidatedNativeProgramEntrySettlement>`.
4. Decides: `selected_entry == plan.entry`, plan psi identity equals artifact identity, retained target context equals recomputed (:43-61); lowered attachment and `structural_parameters` equal the Terminal machine's (:68-83); a receiver is needed iff schema is `ProgramStorageApplication` or private entry RAM is non-empty (:84-89).
5. Cases by shape: with no receiver and non-empty access-view establishments it errors (:113-119); with no receiver and none, `Ok(None)`.
6. Same job elsewhere: none seen.
7. Unused: `admit_executable_entry` called in `driver/mod.rs` (see S2/admit_providers item 7).
8. Stated limits: header :1-8 lists the checks.
9. Fail-open: none seen. (`Ok(None)` at :88 means no receiver settlement for entries that need none; stated by the header.)

### S2/lower.rs (28 lines)
Crate root: declares modules, re-exports (:10-16), imports (:18-25), `test_support` under `#[cfg(any(test, feature = "test-support"))]` (:27-28). Nothing decides.

### S2/operations.rs (1950 lines)
1. Purpose: lowers each reachable `TerminalMachine` to `AbstractFunction`s: per-operation routing, terminator lowering, then assembles `AbstractOperationPlan`.
2. Shape: `LoweringError` (:17-118, 100 variants); nested modules `calls::lower` (:167), `effects::lower` (:437), `routing::lower` (:627), `terminator::lower_terminator` (:1465), `machine::lower_machine` (:1731); entry `lower_selected_module` (:1851).
3. In/out: reads `TerminalModule`, optional `TargetBoundReach` scope, `AdmittedScalarApplications`, `AdmittedTargetLayouts`; produces `AbstractOperationPlan`.
4. Decides: routing is by `OperationKind` variant (:640-1365). Scope filter: `scope.is_none_or(|scope| scope.contains_machine(id))` (:1913), same for boundaries (:1933) and candidates (:1941); `None` means "whole module". Entry machine must exist (:1862); unclosed scalar conditions refused (:1859); `PendingNormalCompletion` is an error (:1474). Jump arity is checked against target block parameters (:1517-1520, :1545-1552, :1590-1593).
5. Cases by shape: `SelectImplementation` result must be a `u64` unsigned integer (:191-192); `SelectMachine` result must be `Callable` (:177-181); `StorageAddress/Alignment` source access must be `SharedBorrow` (:798); `ReturnStructural` only when `machine.result.structural().is_some()` (:1664-1678); `unreachable!("... router is exhaustive")` at :422, :607.
6. Same job elsewhere: the `OperationResult` to `Abstract*Result` match is written three times in `calls::lower` (:217-230, :254-267, :384-398); `runtime_cleanup_actions` (:1713) is an identity copy (its `_module` argument is unused).
7. Unused: `LoweringError` is `pub` through `lower.rs`? (`pub use error::LoweringError` :1846); `lower_selected_module` is `pub(crate)` with callers `selected_input.rs:366` and `test_support.rs:23`.
8. Stated limits: names like `UnsupportedDynamicDispatch`, `UnsupportedAtomicAccess`, `UnsupportedScalarArray` (:49,57,32).
9. Fail-open: (a) `scope: None` lowers every machine (:1913). Class: traced: production paths `lower_native_execution`/`lower_native_callable` always pass `Some(scope)` (`S2/selected_execution.rs:56-66`, `:17-27`); `None` is used by `test_support` (`:23-30`, `:47-60`). (b) `expect_scalar()` calls (:298,415) panic rather than refuse on a malformed result.

### S2/operations/scalars.rs (655 lines)
1. Purpose: lowers integer arithmetic, boolean, IEEE float, bitwise, comparison, conversion, shift, trapping-integer, and scalar-application operations.
2. Shape: submodules `arithmetic` (:9), `boolean` (:155), `ieee_float` (:199), `integer_bitwise` (:281), `integer_constants_and_relations` (:342), `integer_conversion` (:381), `scalar_applications` (:462), `shifts` (:529), `trapping_integer` (:604).
3. In/out: reads `Operation`, `value_types` map; returns `AbstractOperation`.
4. Decides: result must be an integer type (e.g. :92), shift count type taken from `value_types` (:588), `ScalarRepresentationView` classified by `ScalarRepresentationViewPair::classify(source, result)` (:442-448). Scalar applications: callable, host address, undetermined NaN values are refused (:487-508).
5. Cases by shape: comment :488-489 "Source scalar applications currently publish primitive results. Callable results also need retained code and call dependencies." Error variant for (Wrapping|Exact, Add) share `VerifiedWrappingAddMalformed` (:115).
6. Same job elsewhere: none seen.
7. Unused: none checked beyond `pub(crate)` modules routed from `operations.rs`.
8. Stated limits: :488-489 as quoted.
9. Fail-open: `arithmetic::lower` (:92-96) and `integer_constants_and_relations::lower` (:348-378) check only result type, not operand types against the result (`IntegerEqual`, `IntegerLessThan` get no type check). Class: open (the error names `Verified*Malformed` suggest verification was done earlier; I did not trace).

### S2/operations/storage.rs (1861 lines)
1. Purpose: lowers structural, primitive-storage, borrowed-window, leaf-copy, scalar-field and atomic operations, including the backward reaching analysis for atomics.
2. Shape: `unique_row` (:13); `atomic_events::lower` (:36) with `reaching_fact` (:217) and `latest_write` (:327); `borrowed_windows::{lower_move :479, lower_store :569, borrowed_window_root :667, source_signature :785, block_parameter :847}`; `primitive_projection` (:863); `primitive_storage::{parameter :922, owned_state_storage_parameter :944, local :965, lower :1004}`; `structural_establishment::lower` (:1078); `structural_leaf_copy::lower` (:1282); `structural_scalar_fields::{lower :1446, store_destination :1481, lower_store :1536, owned_storage_destination :1670, lower_scalar_read :1690, readable_source_type :1743, exact_parameter :1780}`.
3. In/out: reads machine places, parameters, structural types, `terminal_semantics` helpers; returns `AbstractOperation`.
4. Decides: places are identified by `PlaceId` plus a unique-row check (`unique_row`, :13-20). Atomic coherence: `reaching_fact` walks predecessors backwards; any `Reaching::Unknown` or an empty set refuses (`UnsupportedAtomicCoherence`, :163,:180). Window and store checks key on access (`StructuralAccess`), multiplicity, qualification equality (:529-551, :618-645).
5. Cases by shape: byte-sequence literal only `BorrowedView` carrier (:1181-1188); `EstablishTrivialAffineLocal` needs a record with zero fields and matching ordinal (:1206-1209); atomic `CompareExchangeOnce` needs a plain-result sum type (:125-140); leaf copy `byte_carrier_leaf_copy` only for fixed `u8` arrays with a literal length (:1377-1423); `readable_source_type` rejects write-only borrows and linear sources (:1743-1778). Header comment :6-7 "sequentially consistent atomic memory operations" while the code handles several orderings (`ordering_is_legal`, :152).
6. Same job elsewhere: parameter-by-place lookup is written four times: `primitive_storage::parameter` (:922), `borrowed_windows::block_parameter` (:847), the first two arms of `source_signature` (:789-806), `structural_scalar_fields::exact_parameter` (:1780). `structural_establishment::structural_result` (:1255) duplicates the top-level `structural_result` (`operations.rs:135`) with a different error.
7. Unused: all `pub(crate)` inside private modules; no grep run.
8. Stated limits: none quoted beyond variants named `Unsupported*`.
9. Fail-open: none seen. (`latest_write` falls back to `Reaching::Unknown`/`Effect` conservatively, :387,:395,:425.)

### S2/selected_execution.rs (78 lines)
`lower_native_callable` (:6) and `lower_native_execution` (:46) call `NativeScopePreparation::prepare`, then `lower`/`lower_with_private_entry_ram`, then `bind_selected_provider_coordinates`. Decides only that `checked.require_source(proposal.scope_source())` agrees (:55). Doc comment :42-45 says "settled before Omega 00 starts". Fail-open: none seen.

### S2/selected_input.rs (1075 lines)
1. Purpose: prepares native lowering: selects target layouts and scalar demand, lowers via `lower_selected_module`, wraps the result as `VerifiedNativeArtifactInput`, validates access-view establishments, binds scalar applications.
2. Shape: `ArtifactLoweringError` (:7), `establish_native_access_view_inputs` (:97), `VerifiedNativeArtifactInput` (:186), `NativeExecutionSelection` (:192), `NativeScopePreparation` (:219, `prepare` :230, `lower` :295, `lower_with_private_entry_ram` :482), `bind_selected_provider_coordinates` (:537), `NativeScalarApplications::bind` (:971). 3 tests: one `#[ignore]` (:627-629), two at :883, :935.
3. In/out: reads authorization, `TargetProfile`, optional scope, `AdmittedTargetLayouts`, completed scalar applications; mutates a clone of structural types with resolved geometry (:355-365); produces `VerifiedNativeArtifactInput`.
4. Decides: access-view establishments must match entry `access_view_inputs` rows exactly (:109-120), referent validity from `validate_access_view_referent`, and exclusive views must not overlap by `opaque_identity` and path prefix (:157-165). Scalar closure must name the same artifact, target and layouts (:985-993), and the provider installation custody must equal (:1042).
5. Cases by shape: private entry RAM recipes must be for the same terminal and same layouts (:489-506).
6. Same job elsewhere: none seen.
7. Unused: `prepare`/`prepare_with_private_entry_ram` are `#[cfg(any(test, feature="test-support"))]` (:424,:443).
8. Stated limits: none quoted.
9. Fail-open: (a) :994-1003 only machines that have an authored `declaration` are checked against the completed scalar closure. Class: open. (b) :1021 `let Some(model) = inputs.address_model() else { continue }` skips the address-model comparison when no model exists. Class: open. (c) `scope: None` full-module preparation (:268-283) is reachable only from test-support `prepare` and `test_support.rs` (traced as in operations.rs).

### S2/test_support.rs (67 lines)
`lower_artifact` (:15) lowers a fixture's default entry with `scope: None`; `admit_native_realization` (:43) prepares and admits with an empty `NativeExecutionSelection`. Only compiled under `test`/`test-support`.

---

## Stage 03 files (all under S3/)

### analyses.rs (174 lines)
1. Purpose: catalog of analysis products, lazy per-revision computation, revision binding.
2. Shape: `AnalysisProduct` (:30, 15 variants), `compute_analysis` (:50), `AnalysisRevision` (:116, `bind` :122, `require_all` :130), `analysis_index` (:151).
3. In/out: computes each product from a `PsiOptimizationUnit`; cached in an array indexed `kind as usize - 1`.
4. Decides: `bind` fails when the stored unit identity differs from the recomputed one (`validate_unit_identity`, `control_flow_analyses.rs:44-55`). An `AnalysisKind` not in the match returns `None`, which becomes `UnsupportedAnalysis` (:90, :143).
5. Cases by shape: fixed list of 15 analysis kinds (:54-90).
6. Same job elsewhere: none seen.
7. Unused: `impl AnalysisProduct {}` is empty (:48).
8. Stated limits: none.
9. Fail-open: none seen (unknown kind refuses).

### control_flow_analyses.rs (587 lines)
1. Purpose: control-flow graph, dominators and post-dominators, SCC block components, loop regions, call graph, analysis-root validation.
2. Shape: `analysis_root::{AnalysisRoot :20, validate_unit_identity :44, validate_analysis_root :57}`, `call_graph::call_graph` (:88), `components::block_components` (:190), `dominance::{block_dominates :221, dominators :231, fixed_point_dominators :250}`, `graph::control_flow` (:320), `loops::loops` (:409); data types (:501-587).
3. In/out: reads `PsiOptimizationUnit`; builds Vec-of-rows with binary search by machine id (`machine_row`, :536).
4. Decides: edges come from `block.successors()` ("The representation owns edge construction ... Analyses must not infer a smaller graph from opcodes", :327-328). Exits: `Return*`/`ReturnStructural` is `Normal`, `Crash` is `Crash`, every other terminal has no exit (:351-358). Post-dominator roots are reachable blocks with an exit (:261-267). Loop region: SCC with more than one block or a self edge; irreducible if more than one entry or no dominating header (:456). Call graph keeps `unresolved_calls` when a callable invocation row is missing or `targets_are_resolved()` is false (:125-131).
5. Cases by shape: exits only from the last node being `Return*` or `Crash` (:351-358); `ExternalCompletion` and `InvokeNoNormal` get no exit.
6. Same job elsewhere: a second edge builder `ranked_cycles_validation.rs:701-733` matches `Jump|Conditional|StructuralCase` on the last node by opcode (contradicts the comment at :327-328 for that code). `optimization_graph`/`terminal_graph` grepped: used only inside `ranked_cycles_validation.rs` (768, 819).
7. Unused: `pub(crate) fn by_key` (:494) used by countdown_analyses (read).
8. Stated limits: none.
9. Fail-open: none seen. (`Tarjan` in `mod.rs:200` ignores edges to nodes absent from the graph.)

### control_flow_analyses_tests.rs (540 lines)
Eight `#[test]` functions (grepped count 8). They build `case_plan()` (:61), a five-block plan with a structural case, and pin: reachability and dominance through case edges (:416-444), liveness through case edges (:447-472), jump/conditional/case graph equality (:475-506), place origins (:509-514), a back edge making a loop (:517-540), publication keeping block ownership after node removal (:284-297), `AdjacentBlockMergeRule` keeping the discriminator's checked site (:300-413), constant ranges and rejection of forged bounds (:191-279).

### control_flow_cleanup.rs (1749 lines)
1. Purpose: seven control-flow rewrite rules plus shared provenance-accounting helpers.
2. Shape: rules `ConstantConditionalFoldRule` (:265), `LinearEmptyBlockThreadRule` (:462), `PathQualifiedEmptyBlockThreadRule` (:909), `AdjacentBlockMergeRule` (:22), `NonAdjacentBlockMergeRule` (:693), `SharedJumpFusionRule` (:1075), `UnreachablePrivateMachinePruneRule` (:1255); `built_in_registrations` (:1439, ids 0..6, NonAdjacent last at 6); helpers `scalar_cleanup_functions` (:1590), `terminal_jump` (:1609), `merge_candidates` (:1647), `threadable_empty_blocks` (:1706), `thread_accounting_tail` (:1726), `merge_boundary_ownership_witness` (:650).
3. In/out: each rule reads analyses (`ControlFlowGraph`, `Dominators`, `UseDefinition`, `OwnershipFrontiers`, `CallGraph`, `PostDominators`, `ScalarConstants`) and returns `PsiRewriteCandidate`s (never mutates).
4. Decides: merge needs a unique-entry target (`merge_candidates`, :1662-1667), substitution legality via `replacement_dominates_parameter_uses` (:247-252), ownership frontier witness (:650-691). Fold needs a boolean constant fact (:314). Prune keeps entry, dispatch-table rows, provider candidates, and attached functions, then call-graph and `operation_machine_references` closure (:1343-1380).
5. Cases by shape: only functions with no block structural parameters are touched by merge/thread/fusion (`scalar_cleanup_functions`, :1590-1599); a conditional fold is skipped when any removed block has structural parameters (:336-340); shared-jump fusion only when target is a single terminal `Return*/Crash` node (:1130-1142); path-qualified thread only if all incoming edges have empty cleanup actions (:970-975).
6. Same job elsewhere: `shared_jump_fusion` builds substitutions inline (:1155-1173) the same way as `block_merge_substitutions::merge_substitutions` (:233-263) but without the dominance check.
7. Unused: `merge_boundary_ownership_is_identity`/`_witness` grepped 9 occurrences in this file only (used by three rules).
8. Stated limits: none quoted.
9. Fail-open: none seen at rule level (rules propose; the pass manager validates each candidate with `validate_psi_rewrite_candidate`, `pass_manager.rs:643`). `merge_boundary_ownership_witness` returns an empty witness when no ownership facts exist and the function has no structural parameters, entry claims or declared places (:663-668).

### countdown_analyses.rs (1606 lines)
1. Purpose: three chained analyses of unsigned countdown loops: loop summary, invariant constants (zero and one), and their placement in the preheader. Each computes by `propose` and re-derives by `reconstruct`, and accepts only if both agree.
2. Shape: `countdown_induction` (:12; `analyze_counted_loops` :276), `countdown_invariant_constants` (:339; `analyze_countdown_invariant_constants` :627), `countdown_invariant_constant_placement` (:884; `analyze_countdown_invariant_constant_placement` :1457).
3. In/out: reads `PsiOptimizationUnit`, `ValidatedOptimizerCycleComponents` (certificates), earlier analyses; returns `Validated*` wrappers.
4. Decides: certificates keyed on `CycleComponentId` (machine plus internal edges); constants located by `psi_operation` equal (propose, :440) or by first provenance `PsiProvenance::Operation(id)` (replay, :812); consumer node matched by operation id and operand ids (:1546-1605).
5. Cases by shape: exactly one entry edge and exactly one exit edge (:65-70), preheader ends in a `Jump` to the header (:75-93), two roles `PositiveGuardZero` and `BackedgeDecrementOne` (:637-641), guard is `IntegerLessThan(zero, rank)`, decrement is `ExactIntegerSubtract` (:1553-1602).
6. Same job elsewhere: `compute::summary` (:59-105) versus `replay::reconstruct_one` (:199-246) is a deliberate duplicate (header :5-7 "independent replay"); they differ: propose takes the first matching binding `.find` (:84-88), replay requires exactly one (:225-231).
7. Unused: re-export `pub(crate) use ...control_flow_analyses::*` (:10) exposes everything; consumers in `pass_manager.rs:1284-1290` (read).
8. Stated limits: none quoted.
9. Fail-open: none seen. Mismatch yields `SnapshotMismatch`/`ComponentRosterMismatch`.

### global_value_numbering.rs (2421 lines)
1. Purpose: 16 rules: same-block CSE, dominator GVN and phi-translated GVN each over three key kinds (total, proof-certified, compatible-policy), plus 7 obligation-free identity rules.
2. Shape: key enums `TotalScalarExpressionKey` (:616), `CompatiblePolicyScalarExpressionKey` (:640), `ProofCertifiedScalarExpressionKey` (:790); `propose_same_block_scalar_cse` (:1818), `propose_dominating_scalar_gvn` (:1936), `propose_phi_translated_scalar_gvn` (:2097), `collect_phi_translated_incoming` (:948), `propose_total_scalar_identities` (:1168), `exact_pure_scalar_effect` (:1770), registrations (:1709-1727, ids 0..15). One test at :1663.
3. In/out: reads `UseDefinition`, `EffectSummaries`, `Dominators`, `ScalarConstants`, `unit.accepted_obligation_facts`; returns candidates.
4. Decides: keys are `(op shape, IntegerType, ValueId...)`; commutative ops use `canonical_pair` (:607). A node participates only if its `EffectSummaries` row has `class == PureScalar` and all four knowledge fields `No` (:1777-1787); absent row means not pure. Proof-certified rows need an accepted obligation fact (`accepted_obligation_fact`, :2224,2230). Compatible-policy leaders are rejected if the function has an `OperationObligationReference` for the op (:2240-2245).
5. Cases by shape: no floating-point or representation-view operations have keys (`total_scalar_expression` :313-556 match list); division and remainder have no total or compatible-policy key (:181).
6. Same job elsewhere: the three `propose_*` closures (:2256, :2291, :2326) have identical emit shape; the "result has a use" scan `use_definitions.uses.iter().any(...)` appears at :976, :1209, :1878, :1995 here and at `proof_check_elision.rs:56,147,893`.
7. Unused: `pub(crate) fn phi_translated_contract` (:1763) just calls `dominating_contract`.
8. Stated limits: none quoted.
9. Fail-open: `compatible_policy_cse_phi_rows` drops redundants located in the function entry block (:2416-2419) while the other two row sets do not. This is not accepting; it is an asymmetry I could not explain from the code.

### mod.rs (244 lines)
1. Purpose: crate root of the stage: entry `optimize_abstract_operations` and a Tarjan SCC helper.
2. Shape: `optimize_abstract_operations` (:122), `run_psi_pipeline_for_projection` (:149), `strongly_connected_components` (:176), `AbstractOptimizationError` (:112).
3. In/out: builds a unit with `WorkSchedule::CURRENT.identity()` (:134), runs registries, publishes.
4. Decides: refuses if the selection has any `AbstractOperations` phase selection (:129-133); projection complete selection must equal selections identity (:155); with an empty Psi selection the published plan must equal the input plan (:141-145). `steps.is_empty()` skips cycle-analysis session creation (:160-164).
5. Cases by shape: phase `AbstractOperations` must be empty (:129).
6. Same job elsewhere: none seen.
7. Unused: re-exports.
8. Stated limits: header comments naming stages "Omega 01/00/02" (:4,25,29).
9. Fail-open: `strongly_connected_components` silently skips edges to missing nodes (:200). Class: open; recursion depth equals path length (`visit` recurses, :189-227).

### pass_manager.rs (1594 lines)
1. Purpose: runs each pass step to a fixed point, charging work budgets, recording decisions, building manifests, chaining the transformation ledger; also holds the external-decision replay machinery and `VerifiedPsiOptimizationSession`.
2. Shape: `accounting::{register_revision :22, build_pass_manifest :38, charge :78, convergence_measure :220}` , `baseline::choose_baseline` (:333), `execution::{run_registries :385, run_registries_inner :402, run_unit_inner_with_retention :579, run_loop_invariant_scalar_motion_step :757}`, `external_policy::*` (:1034), `VerifiedPsiOptimizationSession` (:1329), `OptimizationRun` (:1451), `OptimizationRunError` (:1505).
3. In/out: reads `VerifiedPsiOptimizationSession`, `PsiPassStep`s; writes commits, decision logs, `pass_manifests`, `transformation_ledger`.
4. Decides: per iteration, the first rule (registry order) whose validated candidates yield a `Choose` wins (:693-701); `choose_baseline` picks min `(predicted_cost_delta, candidate)` and only if delta `< 0` (:333-344). Convergence: `convergence_measure` must strictly drop each commit (:735-740) or `NonDecreasingConvergenceMeasure`. The measure is selected by comparing `registry.pass()` against six hard-coded canonical byte strings, `copy-propagation.v1`, `control-flow-cleanup.v13`, `dead-pure-scalar-elimination.v2`, `proof-check-elision.v12`, `global-value-numbering.v14`, `state-specialization.v1`, `representation-specialization.v1` (:224-243); any other pass uses `integer_evaluation_operation_count` (:259). Revisions repeated fail `OscillatingRevision` (:27-33).
5. Cases by shape: `PsiPassStep::{RewriteRegistry, LoopInvariantScalarMotion}` (:451-470).
6. Same job elsewhere: the six pass-name literals at :224-243 duplicate constants in `rules.rs:10-19` (grepped `omega\.psi-pass\.` in stage 03: each of the 6 appears in both files with equal versions today; `sccp` and `loop-invariant-scalar-motion` (`ranked_rewrites.rs:1181`) are not in the measure table). `run_unit_inner_with_retention` (:579-755) and `run_loop_invariant_scalar_motion_step` (:757-929) carry parallel decision-recording and commit code.
7. Unused: `run_registries_inner` (:402) takes `supplied_external_decisions: Option<ExternalDecisionLog>`; grepped `run_registries_inner|supplied_external_decisions` in `source-rust/`: 5 lines, all in this file; its only call (:392-399) passes `None`. `ExternalDecisionReplayCursor` is constructed only from that `Option` (:433-439), so replay is unreachable through the public entry `run_registries`. `build_pass_manifest`/`choose_baseline`/`validate_exact_ranked_cycles` grepped together: 10 occurrences in 3 files.
8. Stated limits: doc at :530-536 describes an `OMEGA_PROOF_MEASUREMENTS` census; the env var is read in the production path (:709) and prints to stderr.
9. Fail-open: `build_pass_manifest` returns `Ok(None)` when `step.pass()` is `None` (:46-48), and the caller then fails `MissingPassManifest` (:475). Class: traced. A pass whose version string changed in `rules.rs` but not at :224-243 falls to the default measure; wrong measure leads to `NonDecreasingConvergenceMeasure` or a weaker bound, not acceptance (traced to :735-740).

### proof_check_elision.rs (948 lines)
1. Purpose: 12 rules that remove checked integer operations (identity, zero, self-operation, remainder by 1/-1, negative-one shift) when the operation has an accepted obligation fact, plus dead proof-certified scalar elimination.
2. Shape: `identity_rewrite::{propose_proof_certified_integer_constants :40, propose_proof_certified_scalar_identities :101}`, rules (:205-840), registrations (:842-869, ids 0..11), `propose_proof_certified_remainder_by_constant` (:871). One test at :676.
3. In/out: reads `ScalarConstants`, `UseDefinition`, `EffectSummaries`, accepted obligation facts; emits candidates.
4. Decides: value constants matched by `literal_integer_constant(machine, operand)` equal to expected law value (:138-142); effect row `is_pure_scalar_baseline` (:64,120,912); a missing effect row or obligation fact skips (`is_none_or`, `Ok(..) else continue`).
5. Cases by shape: fixed-width carriers only for self-divide, remainder-by-constant, negative-one shift (`carrier() == IntegerCarrier::Fixed`, :659,:891); signed only for the two negative-one rules (:224-228, :892); self-divide replaces `x/x` by `1` only if `1` is admitted in the type (:660, test :677-705).
6. Same job elsewhere: unsigned-vs-signed constants and "result has a use" scan repeat in GVN (see above).
7. Unused: none checked.
8. Stated limits: none quoted.
9. Fail-open: none seen. Self-remainder (:728-739) and self-subtract (:763-778) do not test the divisor/operand for anything; the accepted obligation fact is the only guard (:67-71).

### publication.rs (317 lines)
1. Purpose: turns a finished `OptimizationRun` into an `AbstractOperationPlan` plus a sealed pre-physical manifest.
2. Shape: `publish_optimization_run` (:196), `ValidatedOptimizedAbstractPlan` (:233, `plan` :286, `into_lowering_input` :293), `source::project_plan` (:113), `validate_function_roster` (:136).
3. In/out: reads the run session; produces `AbstractOperationPlan` with pruned machines removed.
4. Decides: function roster must be the active functions plus pruned machines in the original order, each pruned entry matched by ordinal and machine (:140-160); parameter definition sites must be exactly `FunctionParameter(i)`/`BlockParameter{..}` (:164-176); `psi_selections` must equal the selections' Psi phase (:199).
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Unused: none.
8. Stated limits: none.
9. Fail-open: none seen.

### ranked_cycles_validation.rs (931 lines)
1. Purpose: rebuilds the cyclic components of a unit from Terminal Psi and from the abstract unit, checks they agree, and rederives countdown ranking certificates from both sides.
2. Shape: `components::cyclic_components` (:18), `countdown_ranking::{current::derive :203, current::derive_from_header :239, terminal::project :418, terminal::project_one :431, rederive_exact_certificates :612}`, `graph::{terminal_graph :686, optimization_graph :701}`, `ordinary::rederive_components` (:747), `topology` (:795), `validate_exact_ranked_cycles` (:876), `ValidatedOptimizerCycleComponents` (:888), `ValidatedOptimizerRankingCertificates` (:915).
3. In/out: reads `TerminalModule`, `PsiOptimizationUnit`, plan functions; returns snapshots.
4. Decides: components must be equal between Terminal and unit (`current != terminal` refuses, :769-771). A component gets a countdown certificate only if `terminal::project` finds one (:620-624); components that do not match are left uncertified, not refused. The certificates derived from the unit must equal those projected from Terminal (:631-639).
5. Cases by shape: `derive_one` demands exactly one header candidate (:230-235); `project_one` demands `TerminalRankedScc::Natural`, unsigned rank, exactly one `Strict` edge (:435,:464,:480); header must end in `Conditional` with `IntegerLessThan(zero, rank)` (:246-263,:510-516); decrement must be `ExactIntegerSubtract` with obligation (:324-335, :548-555).
6. Same job elsewhere: `optimization_graph` (:701-733) is a second edge builder besides `control_flow_analyses::graph::control_flow` (:320); it matches `Jump|Conditional|StructuralCase` only (:707-731).
7. Unused: grepped `optimization_graph|terminal_graph` (the local ones): used only at :768 and :819.
8. Stated limits: none.
9. Fail-open: `ordinary::rederive_components` iterates `seed_functions` (the plan's functions) and calls `unit.function(machine.id).ok_or_else(invalid)` (:767); a Terminal machine with no cycles is skipped before that lookup (:764-766). Not a fail-open; noted so a reader knows pruned cyclic machines would be refused here (inferred; I did not read the caller in `validation.rs`).

---

## Coverage
- Read in full: all 34 ranges of `units/unit_007.tsv`, 21,253 lines (Stage 01 files 3,932 lines; Stage 02 files 6,210 lines; Stage 03 files 11,111 lines).
- Skimmed: none.
- Not read: none.
- Greps run (all with the Grep tool): `plan_provider_execution_inputs` (repo, `.rs/.md/.toml`, head 20), `direct_compiler_primitive_leaves` (6 files), `run_registries_inner|ExternalDecisionReplayCursor|supplied_external_decisions` (12 occurrences, 1 file), `merge_boundary_ownership_*` (9, 1 file), `entry_calling_policy` (4, 3 files), `RealizedTargetSupply::(unscoped|no_demand|with_folded_program)|.with_folded_program(|resolve_installation_reach` (12, 6 files), `choose_baseline|build_pass_manifest|validate_exact_ranked_cycles` (10, 3 files), `omega\.psi-pass\.` in stage 03, `#\[test\]` per stage (stage 01 dir 23 in 11 files of which 7 in this unit; stage 02 dir 3; stage 03 `abstract_optimization/` 12 in 4 files of which 10 in this unit), four lowering/admission names (27, 10 files), `DirectBoundaryInput` (7, 3 files), `SelectedProviderPlanFacts` (154, 53 files), `optimization_graph|terminal_graph`.

## Five things a maintainer most needs to know about this unit
1. Stage 01 and 02 decide almost everything by comparing identity text (`module.identities.text(x) == Some(y)`, `format!`-built plan names such as `direct-boundary:` at `S1/provider_rows.rs:118`, `direct-source:`/`direct-build:` at `plan_realization.rs:411,486`). The same "does this leaf match that boundary" test is written twice (`plan_realization.rs:607-623`, `realized_supply.rs:257-272`).
2. Several filters keep an item when nothing matches: `S2/admit_providers.rs:205` (settlement with no matching boundary), `:248-251` (non-`BoundaryCall` callbacks), `S1/provider_execution_inputs.rs:149-154` (Build binding naming an unreached requirement is dropped), `S1/target_supply/plan_realization.rs:109-123` (stale calling applications dropped), `S2/selected_input.rs:994-1003,1021` (machines without a declaration, and absent address model, skip checks). None is commented.
3. `plan_provider_execution_inputs` (`S1/provider_execution_inputs.rs:61`) is `pub` and re-exported with no callers in the repo apart from an archived file; `run_registries_inner` (`S3/pass_manager.rs:402`) always runs with `None` for external decisions, so the external-decision replay code (`pass_manager.rs:1169-1256`) is not reachable from the public entry.
4. Stage 03 convergence depends on six pass-name byte strings copied into `S3/pass_manager.rs:224-243` from `S3/rules.rs`; `sparse-conditional-constant-propagation` and `loop-invariant-scalar-motion` use the default or a separate measure. Rule contracts, registrations and "Omega 00/01/02" labels are all in comments and byte literals, not types.
5. Optimization rules only propose; every candidate is re-validated by `validate_psi_rewrite_candidate` (`pass_manager.rs:643`) and the final plan is replayed in `publication.rs`. Countdown analyses compute and re-derive each result and require equality (`countdown_analyses.rs:263-273,608-624,1435-1454`). Rules refuse functions with structural parameters (`control_flow_cleanup.rs:1590-1599`), non-fixed carriers (`proof_check_elision.rs:659,891`) and anything without `PureScalar` effect rows (`global_value_numbering.rs:1777-1787`).

## What you are unsure about
- Whether `Foreign(Err(_))` leaves (`S1/realized_supply.rs:371`) are refused by a later consumer; I traced only `direct_leaves` (`plan_realization.rs:380-403`).
- Whether the unchecked operand types in `S2/operations/scalars.rs:92-96,348-378` are guaranteed by the lowered-form checker; I did not read the checker.
- Whether the omitted entry-block redundants in `compatible_policy_cse_phi_rows` (`global_value_numbering.rs:2416-2419`) are an intended guard.
- Whether `ranked_cycles_validation::graph::optimization_graph` (:701-733) misses any terminator that `block.successors()` reports; I did not read `PsiOptimizationFunction::successors`.
- Callers of `realize_target_supply` and `NativeProgramPlanningError` (in `S1/src/execution.rs`) were not read; counts for `S1/target_supply` re-exports come from grep only.
