# Area 12-native-representations: crate map

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only; nothing was built or run.
Paths are relative to `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`) unless they start with another root.
Files and lines come from `crates.tsv`. "Dependents" counts come from one grep: manifests under `source-rust`, `omega-rust-v2`, `tools` that declare `<crate> = { path ... }`, excluding the crate's own manifest. That count includes dev-dependencies and includes the aggregate manifest `source-rust/subsystems/compiler/psi-to-native/Cargo.toml` when it lists the crate. Dependent names are directory basenames, not crate names. Counts of tests come from grepping `#[test]` in `.rs` files. No crate in this area has a `tests/` directory or a `[[test]]` target; every test is a unit test inside `src/` (grepped).

## 1. What the area does

This area holds the data forms that the native half of the compiler (`psi-to-native`) passes from one numbered stage to the next. The stages live under `psi-to-native/pipeline/NN_*`; this area holds the types they exchange. The forms run in order: a target-bound program (stage 01 output), abstract operations and an optimization unit (stage 02/03), target operations and legalized operations (stage 04), selected instructions (stage 05/06), register homes (stage 07), the post-allocation machine (stage 08), then the framed, bound, encoded and resolved machine forms (stages 09-12) that end in function fragments and a text-section placement that stage 13/14 turn into objects. Alongside the program forms sit small shared packages: operand records, evidence records, optimization rule/identity records, a structural type catalog, function identity, program-entry contracts, and checkers ("admission" crates) that validate an encoding or a frame without importing a producer stage. Input: published Psi artifacts plus target and provider choices. Output: an `FunctionFragmentEmissionPlan` / `RelocationFreeTextSectionPlacement` that later stages place into objects (`R/README.md:8-14`, `R/README.md:73-88`, read).

There are 30 crates. The README of the area (`R/README.md`, read in full) describes families, not crates; Section 4 lists packages it does not name.

## 2. Crates, in pipeline order

### 2.1 target-bound-program (stage 01 output)
- Path `R/target-bound-program`; 7 files, 2032 lines. Lib path `bound_reach.rs` (`Cargo.toml` `[lib]`).
- Purpose: records which machines, boundaries, callbacks and scalar applications stage 01 selected for one native entry, plus the entry's settlement and the provider adapters selected for it. Stage 02 reads it to lower the body.
- Produces `TargetBoundReach` (`bound_reach.rs:31`), `NativeProgramEntrySettlement` / `ValidatedNativeProgramEntrySettlement` (`entry_settlement.rs:280`, `:396`), `NativeRoot`, `NativeScopeSource` (`native_execution_scope.rs:13`, `:55`), `SelectedProviderAdapters` (`provider_bindings.rs:24`), `SelectedCompilerBuiltin` (`builtins.rs:13`), `SelectedProgramEntryCall` (`entry_call.rs:9`). Consumes published Psi records, provider plans and program-entry contracts (from `Cargo.toml`).
- Entry points: `TargetBoundReach::bind` (`bound_reach.rs:45`), `NativeScopeSource::admit` (`native_execution_scope.rs:66`), `SelectedCompilerBuiltin::select` (`builtins.rs:27`), `SelectedProviderAdapters::from_selected_plans` (`provider_bindings.rs:30`), `retain_selected_provider_adapters` (`provider_bindings.rs:77`).
- Depends on (repo crates): psi-provider-selections, target-provider-admission, psi-authorization, psi-entry-memory, program-entry-contracts, psi-publication-records, diagnostics, selected-provider-plans, provider-plans, native-intrinsics, direct-boundary-bindings, provider-correspondence, boundary-applications, calling-conventions, native-execution-reach, psi-target-layouts, semantic-vocabulary, target, terminal-codec, terminal-psi.
- Dependents: 5 (01_admitted-native-inputs-to-target-bound-program, 02_target-bound-program-to-abstract-operations, execution-scope, provider-admission, verified-optimization-input).
- Layout: `bound_reach.rs` reach and scalar demand binding; `entry_settlement.rs` (922 lines) entry settlement; `native_execution_scope.rs` roots and scope; `provider_bindings.rs`; `builtins.rs`; `scalar_demand.rs`; `entry_call.rs`.
- Tests: 1 `#[test]` in one file (grepped). Thin.
- Open work: `OWNER_CODE_CONCERNS.md` entry "Clippy with `-D warnings` cannot lint an application test because four crates it builds carry warnings" names `scalar_demand.rs` here (two needless borrows). TASKS items `PROVIDER-SELECTION-AFTER-TERMINAL` (`TASKS.md:106`) covers the handoff into native admission, which feeds this crate; it does not name the crate.

### 2.2 native-execution-reach
- Path `R/target-bound-program/execution-reach`; 1 file, 51 lines.
- Purpose: the reached machine IDs, boundary IDs and callback occurrences for one entry, split out so direct provider matching and object publication can read them without compiling the artifact-decoding parts of `target-bound-program` (`R/README.md:20`).
- Produces `ExecutionReach`, `ReachedCallback` (`reach.rs:9`, `:16`). Entry points: `ExecutionReach` accessors `machines`, `contains_machine`, `contains_boundary`, `callbacks` (`reach.rs:36-48`).
- Depends on: semantic-vocabulary only. Dependents: 7 (direct-boundaries, execution-scope, image-emission, native-artifact, target-bound-program, target-vocabulary, verified-optimization-input).
- Tests: none. Open work: none found.

### 2.3 program-entry-contracts
- Path `R/program-entry-contracts`; 4 files, 891 lines. Lib path `program_entry.rs`.
- Purpose: the selected `ProgramEntry` signature, its physical arrival contract and storage binding, carried from stage 01 through emission and requested verification.
- Produces `SelectedProgramEntrySignature` (`entry_signature.rs:159`), `ProgramEntryPhysicalContractPlan` (`physical_contract.rs:7`), `SelectedProgramStorageEntryPlan` (re-export `program_entry.rs:13`), trait `HostedReceiverCustody` (`program_entry.rs:17`), `PhysicalEntrySelection` (`program_entry.rs:26`).
- Depends on: service-surface, psi-publication-records, calling-conventions, diagnostics, provider-plans, psi-entry-memory, semantic-vocabulary, target, terminal-psi, wire. Dependents: 5 (01_admitted-native-inputs-to-target-bound-program, entry-planning, image-emission, target-bound-program, verified-optimization-input).
- Tests: 3 `#[test]`. Open work: none found.

### 2.4 function-identity
- Path `R/function-identity`; 2 files, 128 lines.
- Purpose: names a native callback function by its published Psi program plus placement index, with a fixed 42-byte encoding (`src/lib.rs:1-3`).
- Produces `MachineFunctionIdentity` (`src/lib.rs:10`). Entry points: `callback_thunk` (`:16`), `to_bytes` (`:~37`), `from_bytes` (`:~52`), `is_valid`.
- Depends on: wire, terminal-psi. Dependents: 27 (stages 04, 10-14 and most of the later crates: body-facts, bound-framed-machine, image, installation-admission, legalized-operations, machine-code, physical-recipes, register-homes, selected-instructions, target-vocabulary and others; full list from grep).
- Tests: 2 `#[test]` in `src/tests.rs`. Open work: none found.

### 2.5 optimization-records (crate name `optimization-core`)
- Path `R/optimization-records`; 36 files, 3852 lines. The directory name and crate name differ.
- Purpose: rule contracts, work budgets, decision logs, optimization manifests and the SHA-256 identity newtypes that every native optimization stage and later form carry. Re-exports `optimization-selections` types (`src/lib.rs:44-50`).
- Produces `OptimizationRuleContract` (`contracts/rule.rs:15`), `OptimizationWorkBudget` (`contracts/budget.rs:14`), `BaselineDecisionLog`, `ExternalDecisionLog` (`decisions/`), `OptimizationDecisionRecord`, `OptimizationPassManifestRecord` (`manifest/`), about 40 identity types (`identities/`, listed `src/lib.rs:23-42`), and the selection types `OptimizationSelections` etc. Consumes nothing but `optimization-selections` and `wire`.
- Entry points: no functions; callers construct and compare the records and identities.
- Modules: `contracts/` (rule, budget, analysis sets, verdict), `decisions/` (baseline log, external schema with codec), `identities/`, `manifest/`, `record_encoding`.
- Depends on: optimization-selections, wire. Dependents: 28 (stages 01, 03-09, 12-14; analyses, frame-admission, image-emission, isa-aarch64, isa-x86_64, legalized-operations, machine-code, native-artifact, operation-evidence, optimization-unit, optimization-unit-semantics, post-allocation-machine, psi-to-native, published-objects, register-homes, selected-instructions, target-vocabulary).
- Tests: 32 `#[test]` across 9 files (contracts, identities, manifest, external schema). Open work: none found.

### 2.6 operation-operands
- Path `R/operation-operands`; 7 files, 907 lines.
- Purpose: operand records first produced during abstract lowering and carried unchanged into target and machine forms: call targets, value bindings, integer arithmetic policy, structural stores, access lifecycles and references (`src/lib.rs:1-2`).
- Produces `AbstractCallTarget` (`calls.rs:59`), `ValueBinding`, `AbstractResult` (`values.rs:14`, `:48`), `IntegerArithmetic` / `IntegerArithmeticOperator` / `IntegerOverflowPolicy` (`integer_arithmetic.rs:47`, `:32`, `:25`), `StructuralCopyPath`, `StructuralScalarStoreDestination` (`structural_stores.rs:17`, `:29`), `AccessLifecycleOperands` (`access_lifecycles.rs:12`), `AccessReferenceBacking` (`access_references.rs:11`). Entry points: `IntegerArithmetic::evaluate` (`integer_arithmetic.rs:182`), `AccessLifecycleOperands::from_arguments` (`access_lifecycles.rs:35`).
- Depends on: record-storage, semantic-vocabulary, terminal-psi. Dependents: 13 (05, 09, abstract-operations, entry-planning, legalized-operations, machine-code, native-artifact, queries, register-homes, selected-instructions, target-operations, target-vocabulary, verified-optimization-input).
- Tests: 3 `#[test]` in one file. Open work: none found.

### 2.7 operation-evidence
- Path `R/operation-evidence`; 5 files, 325 lines.
- Purpose: where a row came from (`PsiProvenance`), its charged work (`WorkSettlement`), effect links, value definitions, completion claims, ownership frontier facts and installed provider-call evidence (`src/lib.rs`, `provenance.rs:10-60`, `ownership_frontier.rs:12-49`, `provider_calls.rs:4-25`, `completion_claims.rs:10`).
- Depends on: optimization-core, semantic-vocabulary, terminal-psi, wire. Dependents: 18 (02, 04-07, abstract-operations, analyses, foreign-calls, legalized-operations, machine-code, native-artifact, optimization-unit, provider-admission, register-homes, selected-instructions, target-operations, target-vocabulary, verified-optimization-input).
- Tests: 5 `#[test]` in 4 files (they are in the same grep as 2.8: see note there). Open work: none found.

### 2.8 boundary-applications
- Path `R/operation-evidence/boundary-applications`; 5 files, 742 lines.
- Purpose: evidence about which operator applications and opaque-representation applications a boundary covers, and how each is realized.
- Produces `TerminalBoundaryApplicationCoverage` (`boundary_applications.rs:23`), `BoundaryApplicationCoverageIdentity` (`coverage.rs:23`), `BoundaryOpaqueRepresentationApplications` (`opaque_applications.rs:22`), `BoundaryApplicationRealization` (`realization.rs:21`), `TerminalBoundaryApplicationRealizations` (`realization.rs:154`).
- Depends on: native-intrinsics, semantic-vocabulary, terminal-psi, wire. Dependents: 7 (01, 02, native-artifact, provider-correspondence, psi-to-native, target-bound-program, verified-optimization-input).
- Tests: the `#[test]` grep for `operation-evidence` (5, in 4 files) covers this nested directory too, because the grep descends; the two counts are one set, not ten tests. Open work: none found.

### 2.9 structural-storage
- Path `R/structural-storage`; 2 files, 277 lines.
- Purpose: one shared catalog of structural type declarations, view plans, access lifecycles, quotients and owned-sequence bounds that native forms point to instead of copying (`R/README.md:128-132`).
- Produces `StructuralTypeCatalog` (`structural_type_catalog.rs:17`). Entry points: `new` (`:107`), `with_access_lifecycles` (`:64`), `with_quotients` (`:89`), `with_owned_sequence_bounds` (`:127`), `with_access_views` (`:136`), `ptr_eq` (`:46`).
- Depends on: semantic-vocabulary, terminal-psi, psi-declaration-encoding. Dependents: 9 (04, 05, abstract-operations, legalized-operations, machine-code, queries, register-homes, target-operations, target-vocabulary).
- Tests: 8 `#[test]` (the grep also counts the nested `queries` directory; see 2.10).

### 2.10 structural-storage-queries
- Path `R/structural-storage/queries`; 5 files, 1222 lines.
- Purpose: queries over the catalog: field projection resolution, address/descriptor joins, owner loans, backing validation for borrowed views.
- Entry points: `structural_storage_projection` (`storage_projection.rs:249`), `structural_borrowed_storage_projection` (`:269`), `resolve_path` (`:342`), `is_address_join` (`address_joins.rs:22`), `loan_access_permitted` (`address_joins.rs:101`), `is_bounded_owner_loan` (`owner_loans.rs:18`), `validate_backing` (`access_reference_validation.rs:16`), `borrowed_view_formation` (`:63`).
- Depends on: operation-operands, semantic-vocabulary, structural-storage, terminal-psi, terminal-semantics. Dependents: 5 (04, 05, 09, image-emission, optimization-unit-semantics).
- Tests: 8 `#[test]` in 2 files (shared grep with 2.9). Open work: none found.

### 2.11 abstract-operations (stage 02 output)
- Path `R/abstract-operations`; 10 files, 3576 lines (the nested crates 2.12-2.14 are counted separately).
- Purpose: the target-neutral executable program the optimizer and target lowering consume: functions made of blocks with operations and edge transfers, plus structural catalog, boundary declarations, provider candidates and callable catalogs (`src/lib.rs:3-19`).
- Produces `AbstractOperationPlan` (`src/lib.rs:29`), `AbstractFunction`, `AbstractOperation` (`operations.rs`), atomic, call, and control-flow forms. Entry points: `AbstractOperationPlan::function`, `::operations`, `::boundary_machine`, `::structural_type` (`src/lib.rs:41-62`), `requires_graph_storage_replay` (`replay_requirements.rs`).
- Modules: `operations.rs` (1656 lines), `atomic.rs`, `calls.rs`, `control_flow.rs`, `values.rs`, `quotients.rs`, `storage_mutations.rs`, `replay_requirements.rs`.
- Depends on: operation-evidence, operation-operands, record-storage, structural-storage, language-core, semantic-vocabulary, terminal-psi. Dependents: 9 (02, 03, 04, image-emission, optimization-unit, optimization-unit-semantics, provider-admission, psi-to-native, verified-optimization-input).
- Tests: 53 `#[test]` in 12 files (grep includes the three nested crates below). Open work: `STRUCTURAL-BORROW-IDENTITY` and `NORMALIZED-ABI-LOWERING` (`TASKS.md`) concern structural loans and ABI lowering that pass through this form; neither names the crate.

### 2.12 verified-optimization-input
- Path `R/abstract-operations/verified-optimization-input`; 15 files, 4993 lines.
- Purpose: what stage 02 hands to stage 03. It holds the lowered `AbstractOperationPlan` with its Terminal module and every admitted selection: providers, callbacks, interrupts, component calls, entry call, scalar applications, assumed bindings, borrowed-storage layouts, evidence seals (`src/lib.rs:3-15`, struct at `:64`).
- Produces `VerifiedPsiOptimizationInput` (`lib.rs:64`), `VerifiedPsiOptimizationContext` (`:390`), `AdmittedLoweringFacts` (`:412`), `OptimizedAbstractBody` (`:427`), `NativeCallableBindings`, `AdmittedRuntimeInput`, `EmissionAdmission`. Consumes `AbstractOperationPlan` and an `AuthorizedTerminalArtifact`.
- Entry points: `VerifiedPsiOptimizationInput::lowered` (`lib.rs:160`), `retain_native_selections` (`:213`), `retain_selected_providers` (`:237`), `validate_behavior_exclusions` (`:252`), `retain_admitted_native_callbacks` (`:290`), `retain_admitted_installed_interrupts` (`:297`), `into_optimized_body` (`:338`), `into_lowering_facts` (`:348`).
- Modules: `settlement/` (boundary application and comparison settlement), `callback_settlement`, `native_proposal`, `runtime_provisions` (717 lines), `hosted_receiver`, `component_inputs`, `borrowed_storage`, `publication/target_replay` (also read by `physical-program`).
- Depends on (19 path deps): target-bound-program, native-execution-reach, psi-publication-records, native-behavior-exclusions, provider-acceptance, behavior-exclusion-requests, native-binding-admission, direct-boundary-bindings, native-intrinsics, target-provider-admission, selected-provider-plans, provider-plans, psi-scalar-applications, psi-provider-selections, psi-entry-memory, psi-target-layouts, operation-operands, target-vocabulary, legalized-operations, optimization-unit. Dependents: 10 (01, 02, 03, 04, body-facts, image-emission, native-artifact, psi-to-native, publication, runtime-publication).
- Tests: 5 `#[test]` in 2 files. Open work: none named.

### 2.13 optimization-unit
- Path `R/abstract-operations/optimization-unit`; 13 files, 14834 lines.
- Purpose: pure data for a rebuildable view of the abstract program: control flow, SSA values, effects, ownership events, facts, ledger and manifest, each sealed with canonical identities. Stage 03 rewrites it; the independent checker is crate 2.14 (`src/lib.rs:3-55`).
- Produces `PsiOptimizationUnit` (`lib.rs:162`), `OptimizedAbstractProgram` (`lib.rs:312`, holds plan, unit, selections, budget, identity bundle, pre-physical manifest), `PsiTransformationLedger`, `PrePhysicalOptimizationManifest`, `DominatorSets`.
- Entry points: `reconstruct_psi_optimization_unit_seed` (`construction.rs:41`), `reconstruct_psi_optimization_function_seed` (re-exported `lib.rs:89`), `PsiOptimizationUnit::function` (`:193`).
- Modules: `construction`, `graph`, `dominance`, `identity` (5662 lines), `ledger`, `manifest`, `observation`, `ownership`, `proof`, `range`, `rewrite` (3190), `structural_places`.
- Depends on: operation-evidence, arena, wire, numerics, optimization-core, abstract-operations, language-core, semantic-vocabulary, terminal-psi, psi-declaration-encoding. Dependents: 8 (03, 04, image-emission, isa-aarch64, isa-x86_64, optimization-unit-semantics, psi-to-native, verified-optimization-input).
- Tests: 14 `#[test]` in 2 files. Open work: none named.

### 2.14 optimization-unit-semantics
- Path `R/abstract-operations/optimization-unit-semantics`; 10 files, 46139 lines (the largest crate in the area; `unit_validation.rs` 18242, `current_ownership.rs` 11455, `candidates.rs` 10799).
- Purpose: independent checker for `PsiOptimizationUnit` and for rewrite candidates. It replays each proposed rewrite from the admitted unit and recomputes definitions, uses, effects, ownership frontiers and service reach. It does not lower Psi or sequence passes (`src/lib.rs:5-60`).
- Produces `ValidatedPsiRewrite` (re-export `lib.rs:238`), `OptimizationUnitValidationError`, `OperationSite` (`error.rs`), `CallRegion` (`current_ownership`).
- Entry points: `validate_psi_optimization_unit_with_admitted_cycle_machines` (`unit_validation.rs:18201`), `validate_psi_structural_catalog` (`:18212`), `reconstruct_ownership_frontier_facts_with_admitted_cycle_machines`, `validate_psi_rewrite_candidate` (`candidates.rs:10462`), `validate_case_membership_specialization_candidate` (`:277`), `validate_field_value_specialization_candidate` (`:3122`), `validate_state_argument_specialization_candidate` (`:10290`), `validate_current_value_range_fact_at` (`current_value_ranges.rs:168`), `operation_place_inputs`, `structural_place_access` (`lib.rs:92`, `:98`).
- Depends on: arena, structural-storage-queries, optimization-core, optimization-unit, abstract-operations, semantic-vocabulary, terminal-psi, psi-declaration-encoding, terminal-work, terminal-semantics. Dependents: 4 (03, 04, image-emission, provider-admission). `optimization-unit` and `abstract-operations` do not depend on it, as the README says.
- Tests: 24 `#[test]` in 6 files; `callable_values/tests.rs` is the separate test module. Open work: none named.

### 2.15 target-vocabulary (stage 04 records)
- Path `R/target-operations/target-vocabulary`; 26 files, 7589 lines.
- Purpose: the records a target program is made of, defined apart from the plan: call contracts, boundary realizations, call-site owners, structural homes, value expressions, scalar operands and calling contracts, with their canonical encoders. Also holds the native publication/execution facts (`src/lib.rs:2-14`; `R/README.md:63-77`).
- Produces `NativePublicationFacts` (`publication.rs`, `publication/emission.rs`), `NativeExecutionFacts` (`execution_facts.rs`), `BoundaryRealization` and about 30 realization records (`boundary.rs`, `boundary/realizations.rs`, `boundary/execution.rs`), `NormalizedForeignCallBinding`, `LegalizedStructuralContract` (`call_contracts.rs`), `LegalizedScalarCall` (`scalar_operands.rs`), `TargetScalarExpression` and friends (`values.rs`, 622 lines), `TargetStructuralHomeRequirement` (`storage.rs`), `NativeCallOrigin` (`operation_records.rs`), `CallSiteOwner` (`provenance.rs`), `LegalizedFunctionAbi` (`function_abi.rs`).
- Entry points: mostly type constructors and `identity/*` encoders (`identity/calling.rs`, `structural.rs`, `structural_types.rs`, `normalized_foreign.rs`); error validators in `call_validation.rs`.
- Depends on: native-execution-reach, operation-evidence, operation-operands, wire, structural-storage, language-core, calling-conventions, function-identity, target, provider-stack-contribution, semantic-vocabulary, terminal-psi, runtime-abi, optimization-core, psi-declaration-encoding, psi-callable-encoding. Dependents: 28 (stages 04-06, 09, 11, 13, 14; analyses, encoding-admission, entry-planning, foreign-calls, host, image-emission, installation-admission, legalized-operations, machine-code, native-artifact, native-bindings, numeric-imports, numeric-intrinsics, physical-recipes, post-allocation-machine, register-homes, runtime-publication, selected-instructions, stack-demand, target-operations, verified-optimization-input).
- Tests: 29 `#[test]` in 14 files. Open work: `TARGET-REALIZED-FLOAT-ARITHMETIC` (`TASKS.md:73`) concerns the float realizations (`IeeeFloat*` records here) but names the targets crate; `NORMALIZED-ABI-LOWERING` (`TASKS.md:287`) concerns normalized foreign call records here.

### 2.16 target-operations (stage 04 output)
- Path `R/target-operations`; 3 files, 1068 lines.
- Purpose: the target-level program: functions, control graphs and operations joined to ABI placements, calling plans and admitted boundary realizations, ahead of legalization.
- Produces `TargetOperationPlan` (`src/lib.rs:28`), `TargetFunction`, `TargetControlGraph`, `TargetControlTerminator` (`control_flow.rs:18`, `:40`, `:97`), `TargetUnitOperation` (`operations.rs:50`, a very large enum), `TargetInstruction` (`:44`). Re-exports all of `target-vocabulary`.
- Entry points: `TargetOperationPlan::function` (`lib.rs:~43`), `TargetUnitOperation::structural_result`, `::structural_result_home` (`operations.rs:561`, `:626`).
- Depends on: operation-evidence, operation-operands, target-vocabulary, structural-storage, language-core, calling-conventions, target, semantic-vocabulary, terminal-psi, runtime-abi. Dependents: 8 (04, image-emission, isa-aarch64, isa-x86_64, post-allocation-machine (dev-only), provider-admission, psi-to-native, verified-optimization-input).
- Tests: 75 `#[test]` in 20 files (the grep covers the two nested crates, so this is the family total). Open work: none named.

### 2.17 legalized-operations (stage 04 output, sealed)
- Path `R/target-operations/legalized-operations`; 15 files, 10328 lines.
- Purpose: the execution plan stage 04 publishes: scalar functions, explicit ABI transport, case dispatch, structural contracts and native geometry. Stage 05 selects instructions from the sealed `PreparedExecutionPlan` and reads nothing behind it (`R/README.md:55-62`).
- Produces `LegalizedOperationPlan` (`src/lib.rs:54`), `PreparedExecutionPlan` (`:109`), `PreparationReceipt` (`:209`), `LegalizedOperationPlanIdentity` (`:245`), `LegalizedExactIntegerOperator` (`legality`, `:263`).
- Entry points: `PreparedExecutionPlan::seal` (`:118`), `callable_invocations*` (`:164-182`), `is_intact` (`:198`), `LegalizedOperationPlan::entry_function`, `::function_abi` (`:86`, `:93`); `native_geometry.rs`, `installed_interrupt_geometry.rs`, `structural_inputs.rs` (2790 lines) answer layout queries from both stages 04 and 05.
- Modules: `control_flow_scalar_graph.rs` (1550), `identity/scalar_graph.rs` (1844), `structural_inputs.rs`, `native_geometry.rs`, `selection.rs` (`PreparedInstructionSelection`), `tests.rs` with `tests/identity.rs`, `tests/scalar_operations.rs`.
- Depends on: operation-evidence, operation-operands, arena, wire, target-vocabulary, structural-storage, language-core, calling-conventions, function-identity, optimization-core, target, semantic-vocabulary, terminal-psi, terminal-semantics, runtime-abi, terminal-wire. Dependents: 8 (04, 05, analyses, image-emission, isa-aarch64, isa-x86_64, psi-to-native, verified-optimization-input).
- Tests: 46 `#[test]` in 6 files. Open work: none named.

### 2.18 selected-instructions (stage 05/06 output)
- Path `R/selected-instructions`; 27 files, 22192 lines.
- Purpose: the program after instruction selection and before register homes: functions and blocks stored as copy-on-write `Arc` entries, virtual registers and operands, instruction kinds, call contracts, machine-effect catalogs, liveness and live ranges, each with canonical identities (`src/lib.rs:3-45`).
- Produces `SelectedInstructionPlan` (`lib.rs:181`), `SelectedProgram` (`:201`), `CurrentSelectedProgram` (`:231`), `StagedOptimizedCustodyCore` (`:239`), `SelectedFunctions` (`functions.rs:55`), `SelectedAllocationInput` (`allocation_input.rs:15`), `MachineEffectCatalog` (`effects/catalog*`), `LivenessPlan`, `LiveRangePlan`.
- Entry points: `SelectedInstructionPlan::function`, `::blocks` (`lib.rs:191`, `:195`); `SelectedFunctions::mutate_preserving_register_prefix` (`functions.rs:201`); `SelectedAllocationInput::with_live_ranges`, `::into_legality` (`allocation_input.rs:~40`, `:64`).
- Modules: `calls.rs` (1719), `effects/catalog/{identity,validation}.rs` (2336, 3601), `effects/program/{encoding,identity}.rs`, `plan_identity.rs` (1954), `live_ranges.rs`, `liveness.rs`, `instructions.rs`, `blocks.rs`, `control_flow.rs`, `atomic.rs`, `fixture.rs`.
- Depends on: operation-evidence, operation-operands, wire, target-vocabulary, function-identity, arena, calling-conventions, optimization-core, register-model, runtime-abi, target, semantic-vocabulary, terminal-wire, terminal-psi. Feature `test-support` (empty; enabled by the ISA crates' tests, `Cargo.toml`). Dependents: 31 (the largest fan-in in the area: stages 04-14, analyses, body-facts, encoding-admission, entry-planning, foreign-calls, frame-admission, image-emission, installation-admission, isa-aarch64, isa-conformance, isa-x86_64, machine-code, native-artifact, post-allocation-machine, psi-to-native, register-environment, register-homes, scalar-encoding, target-aarch64, target-x86_64).
- Tests: 127 `#[test]` in 13 files (includes nested crates). Open work: none named.

### 2.19 normalized-foreign-call-plan
- Path `R/selected-instructions/foreign-calls`; 2 files, 1204 lines.
- Purpose: a shared template/fixup vocabulary for normalized foreign-call instruction sequences that the two ISA crates admit (`src/lib.rs:44-136`).
- Produces `Fixup<Kind,State,Callback>` (`lib.rs:70`), `Template<Fixup>` (`:83`), `Error` (`:47`). Entry points: `Template::new` (`:93`), `admit` (`:136`). `fixture.rs` (1012 lines) is behind the `test-support` feature.
- Depends on: operation-evidence, layout-plans, register-model, selected-instructions, semantic-vocabulary, target, calling-conventions, function-identity, provider-stack-contribution, target-vocabulary, terminal-psi. Dependents: 4 (isa-aarch64, isa-x86_64, target-aarch64, target-x86_64).
- Tests: none in this crate. Open work: none named.

### 2.20 physical-recipes
- Path `R/selected-instructions/physical-recipes`; 11 files, 1049 lines.
- Purpose: stage 09's planned function bodies and entry recipes (frame protocol, callable entry, hosted entry, native entry/foreign bridges, imports, stack switch, Windows internet). A `PhysicalRecipe` can be built only by the producing planner, which holds the private origin value (`src/lib.rs:15-25`).
- Produces `PhysicalRecipe<Origin, Instructions>` (`lib.rs:19`), `PlannedFrameFunction`, `FrameRecipe`, `FrameProtocolInstructions` (`frame_protocol.rs:20`, `:27`, `:10`), `PlannedFunctionBody` (`frame_protocol.rs`, used by `bound-framed-machine`), `NativeEntryBridgePlan`, `NativeForeignBridgePlan`, `PhysicalImportProgram`.
- Depends on: target-vocabulary, function-identity, terminal-psi, arena, calling-conventions, diagnostics, frame-admission, machine-code, post-allocation-machine, register-model, runtime-abi, semantic-vocabulary, target, target-aarch64, target-x86_64. Dependents: 8 (09, 10, 11, 12, 14, bound-framed-machine, entry-planning, image-emission).
- Tests: none in this crate. Open work: none named.

### 2.21 register-homes (stage 07 output)
- Path `R/register-homes`; 17 files, 16900 lines.
- Purpose: physical register homes and allocation-legality constraints as data independent of the allocating algorithm, plus pressure-recovery records and the post-allocation optimization manifest. Each wire envelope checks magic, version and a SHA-256 identity before decoding (`src/lib.rs:3-47`).
- Produces `AllocatedProgram` (`lib.rs:301`, holds `Arc<SelectedInstructionPlan>` and `Arc<RegisterHomePlan>`), `AllocatedProgramRef` (`:316`), `RegisterHomePlan` (`:393`, envelope `OMGRAH\0\0` v7), `FixedPrecoloredSegmentHomePlan` (`:499`), `AllocatedCalleeSavedRequirementPlan` (`:649`), `AllocationPublicationFacts` (`publication.rs:14`), recovery records and `FixedViewCopyPlan` (`recovery/`), `PostAllocationOptimizationManifest` (`post_allocation_manifest.rs`).
- Entry points: `RegisterHomePlan::encode` / `::decode` (`lib.rs:408`, `:414`), `register_home_identity` (`:339`), `AllocatedProgram::as_ref` (`:307`), `AllocationPublicationFacts::retain` (`publication.rs:24`).
- Modules: `constraints.rs`, `recovery.rs` plus `recovery/{structural_calls,structural_contracts,structural_declarations,selected_blocks,selected_instructions,fixed_view_copy_codec,program_contracts,execution_facts,publication}.rs`, `post_allocation_manifest.rs`, `pressure_rematerialization.rs`, `literal_fold.rs`.
- Depends on: operation-evidence, operation-operands, wire, target-vocabulary, register-environment, structural-storage, layout-plans, calling-conventions, function-identity, optimization-core, target, register-model, runtime-abi, selected-instructions, provider-stack-contribution, semantic-vocabulary, terminal-semantics, terminal-wire, terminal-psi. Dependents: 14 (06, 07, 08, 09, 11, 13, analyses, encoding-admission (dev), frame-admission, image-emission, machine-code, post-allocation-machine, psi-to-native, published-objects).
- Tests: 134 `#[test]` in 15 files (`recovery/codec_tests.rs` is 2063 lines). Open work: none named.

### 2.22 post-allocation-machine (stage 08 output)
- Path `R/post-allocation-machine`; 12 files, 3637 lines.
- Purpose: the physical program: machine instructions with chosen alternatives and physical operands, cost model, optimization custody, with a wire codec (`PostAllocationMachinePlan::encode`/`decode`, `lib.rs:103`, `:107`).
- Produces `PostAllocationMachinePlan` (`lib.rs:66`), `PostAllocationMachineProgram` (`program.rs:18`), `PhysicalFunction` (alias of `SelectedFunction`, `program.rs:35`), `PostAllocationMachineFunction`, `PostAllocationMachineBlock` (`control_flow.rs`), `PostAllocationMachineIdentity` (`identity.rs`), `target_cost_model` (`costs.rs`).
- Entry points: `PostAllocationMachineProgram::function`, `::physical`, `::recomputed_identity` (`program.rs:38`, `:44`, `:51`), `program_validation.rs` (692 lines), `adapters.rs`.
- Depends on: calling-conventions, wire, target-vocabulary, terminal-psi, optimization-core, register-homes, register-model, register-environment, selected-instructions, target, semantic-vocabulary; dev: target-x86_64, runtime-abi, target-operations. Dependents: 14 (08, 09, 11, 12, 13, encoding-admission, entry-planning, frame-admission, image-emission, installation-admission, machine-code, physical-recipes, psi-to-native, published-objects).
- Tests: 37 `#[test]` in 3 files (`codec_tests.rs` 987 lines). Open work: none named.

### 2.23 physical-program (crate name; directory `post-allocation-machine/body-facts`)
- Path `R/post-allocation-machine/body-facts`; 1 file, 132 lines. Directory and crate name differ.
- Purpose: obligations carried with the current machine form after the operation graphs are dropped: `NativeBodyFacts` holds publication facts, private function references and planned installation resources (`src/lib.rs:1-4`, `:14`).
- Produces `NativeBodyFacts` (`:14`), `EmissionFacts` (`:55`), `PhysicalEmissionFacts` (`:62`), `BoundBodyContracts` (`:82`). Entry points: `NativeBodyFacts::retain_selected_body_facts` (`:25`), `::into_framed_body_facts` (`:37`), `BoundBodyContracts::encoding_subject` (`:95`), `::needs_installed_branch_checks` (`:88`).
- Depends on: installation-admission, function-identity, machine-code, selected-instructions, verified-optimization-input. Dependents: 10 (05, 06, 07, 08, 09, 10, 14, entry-planning, image-emission, psi-to-native).
- Tests: none. Open work: none named.

### 2.24 machine-code (stage 09-13 forms)
- Path `R/machine-code`; 35 files, 8431 lines (with 4 nested crates counted separately).
- Purpose: the three machine-code resolution levels: layout-independent encoding, resolved function-relative layout, and emitted fragments with a relocation-free text-section placement; also the framed form entering encoding (`src/lib.rs:8-21`; `R/README.md:79-88`).
- Produces `FramedMachineProgram` (`layout.rs:271`), `EncodedMachineProgram` (`:284`), `ResolvedMachineProgram` (`:300`), `OptimizedMachineLayout` (`:323`), `SelectedFormEncoding` (`encoding.rs:34`), `ResolvedMachineLayout` (`layout.rs:186`), `FunctionFragmentEmissionPlan` (`fragments.rs:41`), `RelocationFreeTextSectionPlacement` (`layout/text_section.rs:53`), `BoundEncodingRequirements` (`layout.rs:803`), `EncodedBodyPublication` (`:917`), x86 branch relaxation records (`layout.rs:462-500`).
- Entry points: `resolved_machine_layout_identity` (`layout.rs:347`), `recomputed_identity` on encoding/layout/fragments, `BoundEncodingRequirements::new`, `::capture_original`, `::finish` (`layout.rs:811`, `:825`, `:833`), `SelectedFormEncodedRowStore::intern` (`encoding.rs:134`).
- Modules: `encoding.rs`, `encoding_identity.rs` (717), `layout.rs` (945) with `layout/{relaxation_record,text_section,text_section_publication}.rs`, `fragments.rs` with `fragments/{identity,publication}.rs`, `branch_encoding.rs`, `calls.rs`, `boundary.rs`, `storage.rs`, `storage_frames.rs`, `functions.rs`, plus many small origin/requirement modules (`call_origins`, `fragment_origins`, `exit_requirements`, `frame_demands`, `layout_requirements`, `program_context`, `hosted_entry`, `import_code`, `placed_body`). `lib.rs:545` re-exports `native_stack_demand` as `physical_stack`.
- Depends on: native-stack-demand, operation-evidence, operation-operands, register-environment, record-storage, wire, target-vocabulary, structural-storage, register-homes, post-allocation-machine, calling-conventions, function-identity, optimization-core, target, register-model, selected-instructions, provider-stack-contribution, semantic-vocabulary, terminal-psi. Dependents: 21 (09-14, body-facts, bound-framed-machine, encoding-admission, entry-planning, frame-admission, image, image-elf, image-emission, image-macho, image-pe, installation-admission, native-artifact, physical-recipes, psi-to-native, published-objects).
- Tests: 40 `#[test]` in 13 files. Open work: none named.

### 2.25 frame-admission
- Path `R/machine-code/frame-admission`; 3 files, 1205 lines.
- Purpose: checks callee-save storage, frame coordinates and encoded prologue/epilogue arenas against explicit demands and target contracts. Checked records have private constructors; physical construction is elsewhere (`README.md`, `frame_layout.rs:1-3`).
- Produces `ValidatedNonAuthoritativeCalleeSaveStorage`, `NonAuthoritativeCalleeSaveStorageReceipt` (`frame_layout.rs:115`, `:86`), `ValidatedTargetFrameLayout`, `TargetFrameLayoutReceipt` (`:163`, `:133`), `FrameProtocolCodec` (`frame_protocol.rs:5`).
- Entry points: `validate_non_authoritative_callee_save_storage` (`frame_layout.rs:401`), `call_site_stack_contract` (`:274`), `body_writes_link_register` (`:300`), `committed_extent` (`:385`), `stack_commit_granule_bytes` (`:225`), `frame_unwind_policy` (`:236`).
- Depends on: machine-code, optimization-core, post-allocation-machine, register-environment, register-homes, register-model, selected-instructions, target, semantic-vocabulary, target-x86_64, target-aarch64, diagnostics, wire. Dependents: 8 (09, 11, 13, encoding-admission, entry-planning, installation-admission, physical-recipes, published-objects).
- Tests: none in this crate. Open work: none named.

### 2.26 encoding-admission
- Path `R/machine-code/encoding-admission`; 11 files, 5958 lines.
- Purpose: checks current machine encodings, layouts and relaxation records without importing an executable stage: independent byte and branch checks via the ISA decoders, frame equations, contract projections, and replay against explicitly retained originals (`README.md`, `src/lib.rs:1-6`).
- Entry points: `validate_current_selected_form_encoding` (`lib.rs:117`), `validate_optimized_layout_independent_selected_form_encoding` (`:85`), `validate_encoded_integrity` (`:246`), `validate_row`, `validate_size` (`validation.rs`), `validate_current_resolved_layout` (`layout/mod.rs:119`), `validate_optimized_resolved_selected_form_layout` (`:88`), `validate_target_frame_protocol_encoding` (`frame_protocol.rs:19`), `prepare_exit_requirements` (`exit_requirements.rs:18`).
- Depends on: frame-admission, register-environment, optimization-selections, target-vocabulary, calling-conventions, machine-code, post-allocation-machine, isa-aarch64, isa-x86_64 (these two live in stage 11's directory), register-model, selected-instructions, target, semantic-vocabulary; dev: register-homes. Dependents: 5 (11, 12, 13, psi-to-native, published-objects).
- Tests: 15 `#[test]` in 3 files. Open work: none named.

### 2.27 encoded-scalar-plan
- Path `R/machine-code/scalar-encoding`; 2 files, 324 lines.
- Purpose: one shared step program (`Step`, `plan`) for variable-width encoded scalar loads and stores, so x86-64 and AArch64 cannot drift apart (`src/lib.rs:3-26`).
- Entry points: `plan` (`lib.rs:53`); under feature `test-support` (`Cargo.toml`), `replay::execute` (`:96`) and `harness.rs`.
- Depends on: register-model, selected-instructions. Dependents: 2 (isa-aarch64, isa-x86_64). Tests: none counted (the shared harness is behind the feature).

### 2.28 native-stack-demand
- Path `R/machine-code/stack-demand`; 3 files, 452 lines. Lib files `demand.rs`, `physical_stack.rs`, `tests.rs`.
- Purpose: computes a program's stack demand from physical facts alone (no bytes or earlier executable forms).
- Produces `StackDemand` (`demand.rs:179`), `PhysicalStackProgram` (`physical_stack.rs:72`), `StackDemandError` (`demand.rs:14`). Entry point: `derive_physical_stack_demand` (`demand.rs:42`).
- Depends on: function-identity, provider-stack-contribution, semantic-vocabulary, target, target-vocabulary, terminal-psi. Dependents: 2 (machine-code, runtime-publication).
- Tests: 2 `#[test]`. Open work: none named.

### 2.29 bound-framed-machine (stage 10 output)
- Path `R/bound-framed-machine`; 2 files, 82 lines. Lib path `bound_framed_machine.rs`.
- Purpose: a planned function body with its installation resources and the encoding obligations, issued only by stage 10; the issuer capability is never returned (`bound_framed_machine.rs:1-2`).
- Produces `BoundFunctionBody` (`:9`), `BoundEntry<Parts>` and aliases `BoundForeignAdapter`, `BoundHostedEntry`, `BoundPhysicalEntry`, `BoundCallableEntry`, `BoundPrivateEntry` (`entries.rs:21-31`). Entry points: `BoundFunctionBody::from_binding` (`:18`), `::into_encoding_parts` (`:38`), `BoundEntry::from_binding`.
- Depends on: installation-binding-issuer, physical-recipes, function-identity, machine-code. Dependents: 3 (10, 11, psi-to-native). Tests: none.

### 2.30 installation-binding-issuer
- Path `R/bound-framed-machine/issuer`; 1 file, 6 lines. Holds `BindingIssuer` and the constant `BINDER` (`issuer.rs:4-5`); stage 10 holds the capability. Depends on nothing. Dependents: 2 (10_framed-machine-to-bound-framed-machine, bound-framed-machine). Tests: none.

## 3. How data flows through the area

1. Stage 01 binds an entry: `TargetBoundReach::bind` (`target-bound-program/bound_reach.rs:45`), with `NativeScopeSource::admit` and `ExecutionReach`; entry contracts come from `program-entry-contracts`.
2. Stage 02 lowers to `AbstractOperationPlan` and wraps it with `VerifiedPsiOptimizationInput::lowered` (`verified-optimization-input/src/lib.rs:160`), then fills selections through the `retain_*` methods.
3. Stage 03 builds `PsiOptimizationUnit` (`reconstruct_psi_optimization_unit_seed`, `optimization-unit/src/construction.rs:41`), rewrites it, checks each rewrite with `validate_psi_rewrite_candidate` (`optimization-unit-semantics/src/candidates.rs:10462`) and publishes `OptimizedAbstractProgram` (`optimization-unit/src/lib.rs:312`).
4. Stage 04 lowers to `TargetOperationPlan` (`target-operations/src/lib.rs:28`) and seals `PreparedExecutionPlan::seal` (`legalized-operations/src/lib.rs:118`).
5. Stage 05 selects instructions into `SelectedInstructionPlan` (`selected-instructions/src/lib.rs:181`); stage 06 rewrites it and hands stage 07 a `SelectedAllocationInput` (`allocation_input.rs:15`).
6. Stage 07 produces `AllocatedProgram` / `RegisterHomePlan` (`register-homes/src/lib.rs:301`, `:393`).
7. Stage 08 produces `PostAllocationMachineProgram` (`post-allocation-machine/src/program.rs:18`), with `NativeBodyFacts` (`body-facts/src/lib.rs:14`) kept beside it.
8. Stage 09 plans frames and entry recipes (`physical-recipes` `PlannedFunctionBody`), checked by `frame-admission`; the result is `FramedMachineProgram` (`machine-code/src/layout.rs:271`).
9. Stage 10 binds installations and issues `BoundFunctionBody` (`bound-framed-machine/bound_framed_machine.rs:9`) using the `BINDER` capability.
10. Stage 11 encodes into `EncodedMachineProgram` (`layout.rs:284`) / `SelectedFormEncoding` (`encoding.rs:34`); `encoding-admission` validates it and `encoded-scalar-plan` supplies the shared scalar load/store steps.
11. Stage 12 resolves layout into `ResolvedMachineProgram` / `OptimizedMachineLayout` (`layout.rs:300`, `:323`).
12. Stage 13 builds `FunctionFragmentEmissionPlan` and `RelocationFreeTextSectionPlacement` (`fragments.rs:41`, `layout/text_section.rs:53`); stage 14 and later build objects and the image from these. `native-stack-demand` derives stack demand from the physical program for runtime publication.
Shared across all steps: `optimization-core`, `operation-evidence`, `operation-operands`, `structural-storage`, `target-vocabulary`, `function-identity`.

## 4. Where this area's map differs from the documents

- `R/README.md` does not name these packages: `optimization-records` (crate `optimization-core`; grepped 0 hits for the name), `machine-code/scalar-encoding`, `machine-code/stack-demand`, `selected-instructions/foreign-calls`, `post-allocation-machine/body-facts`, `bound-framed-machine/issuer`, `machine-code/encoding-admission`, `machine-code/frame-admission` (each 0 grep hits in the README). `source-rust/README.md:28` points to the directory as a whole.
- `R/README.md:8` (family table) says stage 04 lowers abstract operations to target operations; `R/target-operations/target-vocabulary/src/lib.rs:3` says "Stage 02 lowers to target operations".
- `R/machine-code/src/layout.rs:318-321` ("Stage 09's output ... relaxation trace stays with stage 09") disagrees with `R/README.md:81-83`, which gives stage 12 as the producer of `ResolvedMachineProgram`, and stage 09 as the frame planner. The directory listing agrees with the README (`pipeline/12_selected-form-encoding-to-resolved-layout`).
- `R/selected-instructions/src/lib.rs:41` names `AllocatedRegisterHomePlan`; no such type exists (grepped whole `psi-to-native`). The type is `RegisterHomePlan` / `AllocatedProgram`.
- `R/machine-code/frame-admission/README.md` says "Physical frame construction lives in `physical-planning`"; no crate or directory by that name exists (find, grepped; only the README mentions it). Stage 09 has `entry-planning/` and `src/`.
- `R/target-operations/legalized-operations/src/lib.rs:28-35` lists "four subordinate owners" starting with `legality`; `legality` is an inline `mod` in `lib.rs:261`, not a file, and `selection`, `function_abi`, `native_geometry`, `structural_inputs`, `installed_interrupt_geometry` are not in that list. Its header label "`Psi 06-08 -> Native`" uses no stage numbering from the pipeline directory.
- `R/README.md:55-62` and `R/target-operations/src/lib.rs` say later forms cannot reach `target-operations` or `legalized-operations` through manifests. The manifests agree for `selected-instructions`, `register-homes`, `machine-code` and their followers. `post-allocation-machine` lists `target-operations` as a dev-dependency (`post-allocation-machine/Cargo.toml`), which the README's "check-boundary refuses any path" sentence does not mention; I did not read `check-boundary` to see whether dev-dependencies are exempt.
- Doc comments in `optimization-unit/src/lib.rs` use "Omega 02" and `machine-code/src/lib.rs` uses "Omega 07-09" as stage labels; these do not match the `pipeline/NN_` numbering (`optimization-unit` is built in stage 03; machine-code covers stages 09-13).
- `AGENTS.md` says nothing specific about this area beyond the Psi/Omega firewall; no conflict found.

## 5. What I did not read, and what I am unsure about

Not read in full: the bodies of the big files (`optimization-unit-semantics/src/unit_validation.rs` 18242 lines, `current_ownership.rs`, `candidates.rs`, `register-homes/src/recovery/*`, `selected-instructions/src/effects/*`, `legalized-operations/src/structural_inputs.rs`, `target-bound-program/entry_settlement.rs`, `target-vocabulary/src/boundary/*`). I read their public item lists with grep only. The crates `physical-recipes`, `foreign-calls`, `encoded-scalar-plan`, `encoding-admission` internals were read by heading and public functions only. I did not read the pipeline stage crates that produce these forms, `check-boundary`, or the spec pages these crates cite.

Unsure:
- Dependent counts include dev-dependencies and the aggregate manifest `psi-to-native/Cargo.toml`; they are manifest counts from grep, not a resolved build graph. Several dependents (for example "native-bindings", "host") are directory basenames I did not map to crate names.
- Test counts are `#[test]` greps. For nested crates (`operation-evidence` / `boundary-applications`, `structural-storage` / `queries`, `abstract-operations` and its three nested crates, `target-operations` and its two, `selected-instructions` and its two, `machine-code` and its four) a parent's grep also covers the children, so the same tests appear under both. I could not tell from the counts alone how many belong to each; I did not subtract them.
- `OWNER_CODE_CONCERNS.md` has three entries (read the titles); only one names a crate here (`target-bound-program`). TASKS items cited are by relation, not by naming the crate; no TASKS item names any crate in this area by path.
- `function-identity` entry-point lines marked `~` are approximate.
