//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

/// Assert every `source` omits each listed token, naming the owner that must
/// not spell it.
fn assert_absent(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(!source.contains(token), "{complaint} {token}");
        }
    }
}

/// Assert every `source` still carries each listed token.
fn assert_present(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(source.contains(token), "{complaint} {token}");
        }
    }
}

#[test]
fn final_representation_admission_belongs_to_psi_checking() {
    let coordinator = include_str!("checked/checking/source_checking.rs");
    assert!(coordinator.contains("psi_compiler::ProductRepresentationInput"));
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "rederive_opaque_representation_selections(",
            "OpaqueDataPropertyReceipt::copy(",
            "CheckedSemanticCopy",
        ],
    );
    let checking = include_str!("../../../../psi/compilation/driver/src/source_checking.rs");
    assert!(!checking.contains("pub opaque_properties:"));
    assert!(!checking.contains("pub opaque_representations:"));
    assert!(checking.contains("rederive_opaque_representation_selections("));
    assert!(checking.contains("OpaqueDataPropertyReceipt::copy("));
    let admission = checking
        .find("self.admission.representations.admit(&program)")
        .unwrap();
    let completion = checking
        .find("validation::source_completion::prepare_source_completion(")
        .unwrap();
    let reuse = checking.find("prior.judges_preparation_input(").unwrap();
    assert!(admission < completion && completion < reuse);
}

#[test]
fn product_policy_admission_has_one_source_owner() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "source.typed(",
            "mod exclusions",
            "collect_product_root_grants(",
            "collect_opaque_representation_selections(",
            "native_bindings::collect",
        ],
    );
    let authentication = coordinator
        .find("rejoin_package_provider_selections(")
        .unwrap();
    let source_admission = coordinator.find("source.admit_product_policy(").unwrap();
    let receipt_attachment = coordinator.find("native_binding_evaluation").unwrap();
    assert!(authentication < source_admission && source_admission < receipt_attachment);
    let source =
        include_str!("../../../../psi/compilation/driver/src/source_compilation/product_policy.rs");
    let mut previous = 0;
    for operation in [
        "self.admit_product_provider_requests(",
        "collect_opaque_representation_selections(",
        "native_bindings::collect(",
        "collect_product_root_grants(",
        "product_declarations::admit(",
        "collect_behavior_exclusions(",
    ] {
        let position = source.find(operation).expect(operation);
        assert!(
            position > previous,
            "source operation out of order: {operation}"
        );
        previous = position;
    }
    assert_absent(
        &[source],
        "Psi owns",
        &[
            "ComputedBuildConfig",
            "package_compilation::",
            "effects::",
            "provider_planning::",
        ],
    );
}

#[test]
fn product_declaration_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert!(coordinator.contains("source.admit_product_policy("));
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "mod additional_declarations",
            "resolve_product_path(",
            "WireCompatibilityFact",
        ],
    );
    let admission =
        include_str!("../../../../psi/compilation/compile-time/src/product_declarations.rs");
    assert_present(
        &[admission],
        "Psi admission lost",
        &[
            "request.validate_shape()",
            "resolve_build_request_author(",
            "symbol_package_identity(root)",
            "declared.contains(request)",
        ],
    );
    assert_absent(
        &[admission],
        "Psi admission depends on",
        &["ComputedBuildConfig", "provider_planning::", "effects::"],
    );
    let legacy = include_str!("../../../../psi/compilation/compile-time/src/assumed_bindings.rs");
    assert!(!legacy.contains("collect_executed_assumed_bindings"));
    let compile_time = include_str!("../../../../psi/compilation/compile-time/src/lib.rs");
    assert!(!compile_time.contains("harvest_wire_compatibility_demands"));
}

#[test]
fn source_entry_admission_never_calls_omega_with_typed_trees() {
    let query = include_str!("checked/checking/product_queries.rs");
    assert!(query.contains("&target::ProgramEntrySlots"));
    assert!(!query.contains("provider_planning::"));
    let completion = include_str!("checked/checking/build_continuation.rs");
    assert!(completion.contains("prepared.admit_root_bindings("));
    assert!(!completion.contains("provider_planning::program_entry::collect_root_bindings("));
    let operand = include_str!(
        "../../../../psi/compilation/representations/typed-trees/src/typed_trees/declarations/product_operand.rs"
    );
    assert!(!operand.contains("trait ProductEntryCompatibility"));
    let admission = include_str!("../../../../psi/compilation/checking/src/product_entries.rs");
    assert!(admission.contains("pub fn validate_source_compatibility("));
    assert!(admission.contains("ProductEntryDeclaration::from_declared_owner("));
    assert!(!admission.contains("target::"));
    let realization = include_str!("../../realization/provider-planning/src/program_entry.rs");
    assert!(realization.contains("validate_source_compatibility("));
    assert!(realization.contains("selected.slot.source_contract()"));
    assert_absent(
        &[realization],
        "Omega owns source judgment",
        &[
            "ProductEntryQueryCompatibility",
            "fn core_arrival_requirement_contract(",
            "fn arrival_requirement_contract(",
        ],
    );
    let catalog = include_str!("../../backend/target/src/target_profile.rs");
    assert!(catalog.contains("ProgramEntrySourceCatalog for ProgramEntrySlots"));
    assert_absent(
        &[catalog],
        "target catalog receives",
        &[
            "TypedTrees",
            "ProductEntryDeclaration",
            "NormalizedTypeIdentity",
        ],
    );
}

#[test]
fn product_provider_request_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert!(coordinator.contains("source.admit_product_policy("));
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "prepare_provider_applications(",
            "prepare_product_operand_application(",
            "collect_product_provider_selections(",
            "collect_product_selection_occurrences(",
            "resolve_build_request_author(",
            "ProductApplicationDemand",
        ],
    );
    let continuation = include_str!("../../../../psi/compilation/driver/src/source_compilation.rs");
    assert!(!continuation.contains("pub fn prepare_product_operand_application("));
    let admission = include_str!(
        "../../../../psi/compilation/driver/src/source_compilation/provider_requests.rs"
    );
    assert!(admission.contains("pub(super) fn admit_product_provider_requests("));
    assert!(admission.contains("collect_product_provider_selections("));
    assert_absent(
        &[admission],
        "Psi admission owns",
        &[
            "ComputedBuildConfig",
            "provider_planning::",
            "provider_selection::",
        ],
    );
}

#[test]
fn selected_constant_evaluation_belongs_to_psi() {
    let selection = include_str!("../../realization/selected-dispatch/src/constant_evaluation.rs");
    assert!(selection.contains("build_time_evaluation::SelectedConstantEvaluationRequest"));
    assert_absent(
        &[selection],
        "selection drives",
        &[
            "evaluate_selected_operators(",
            ".finish(typed)",
            "fold_data_layout_queries(",
        ],
    );
    let geometry = include_str!("../../realization/provider-planning/src/data_layout_queries.rs");
    assert_absent(
        &[geometry],
        "geometry owns",
        &[
            "expression_mut(",
            "proof_only::",
            "PreCheckEvaluation",
            "reject_unresolved_layout_operands",
        ],
    );
    let evaluation = include_str!(
        "../../../../psi/compilation/compile-time/src/const_evaluation/selected_constants.rs"
    );
    let queries = include_str!(
        "../../../../psi/compilation/compile-time/src/const_evaluation/data_layout_queries.rs"
    );
    assert_absent(
        &[evaluation, queries],
        "Psi evaluation depends on",
        &[
            "provider_planning::",
            "selected_dispatch::",
            "target::",
            "layout::",
        ],
    );
}

#[test]
fn deferred_constants_use_canonical_entries_without_source_snapshots() {
    let checking = include_str!("checked/checking/source_checking.rs");
    let closure = include_str!("../../build-evaluation/src/scalar_completion.rs");
    let product = include_str!("../../products/src/prepared_build.rs");
    assert_absent(
        &[checking, closure, product],
        "coordinator owns",
        &[
            "constant_recipe_source",
            "constant_recipe_continuations",
            "read_constant_recipe_closure(",
            "with_constant_recipe_closure(",
            "plan_laid_layouts =",
            "const_declarations.for_each_mut",
        ],
    );
    assert!(closure.contains("build_time_evaluation::complete_scalar_applications("));
    let checking = include_str!("../../../../psi/compilation/driver/src/source_checking.rs");
    assert!(checking.contains("constant_entries::retain_evaluation_entries(&mut program)"));
    let selection = include_str!("../../realization/selected-dispatch/src/constant_evaluation.rs");
    assert_absent(
        &[checking, product, selection],
        "source snapshot path returned:",
        &[
            "RetainedConstantEvaluation",
            "ConstantValueClosureRequest",
            "close_constants(",
        ],
    );
}

#[test]
fn reused_product_selection_shares_the_checked_program() {
    use std::sync::Arc;

    let program = Arc::new(checked_trees::CheckedTrees::default());
    let mut selections = psi_compiler::ProductSelections::default();
    let first = selections
        .select(
            psi_compiler::CheckedSource {
                program: Arc::clone(&program),
                retained: None,
                reused: false,
            },
            None,
        )
        .expect("first root selection");
    let second = selections
        .select(
            psi_compiler::CheckedSource {
                program: Arc::clone(&program),
                retained: None,
                reused: true,
            },
            None,
        )
        .expect("reused root selection");
    assert!(Arc::ptr_eq(&first.program, &second.program));
}

#[test]
fn canonical_proof_completion_has_no_source_rewrite_api() {
    let coordinator = include_str!("../../build-evaluation/src/scalar_completion.rs");
    assert!(coordinator.contains("build_time_evaluation::complete_scalar_applications("));
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "discharge_all(",
            "CheckedTrees::with_roots(",
            "settle_closed_constant_execution(",
            "closed_facts",
            "decided_formations",
            "machine_authored_target(",
            "declaration_formation_owners(",
            "proof_application_machines(",
            "proof_application_declaration_owners(",
            "target_has_proof_demand(",
        ],
    );
    let facade = include_str!("../../../../psi/compilation/driver/src/lib.rs");
    let stage = include_str!(
        "../../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/lib.rs"
    );
    assert_absent(
        &[facade, stage],
        "source rewrite API returned:",
        &[
            "ConstantProofClosure",
            "ConstantProofApplication",
            "settle_closed_constant_execution",
            "pub use facts::declaration_descriptions::retain_declaration_descriptions",
        ],
    );
    let entries =
        include_str!("../../../../psi/compilation/driver/src/source_checking/constant_entries.rs");
    assert!(entries.contains("fn retain_evaluation_entries("));
    assert!(!entries.contains("CheckedTrees::with_roots("));
    let applications =
        include_str!("../../../../psi/foundation/semantic-vocabulary/src/proof_applications.rs");
    assert_absent(
        &[entries, applications],
        "Psi constant entries or proof applications depend on",
        &[
            "provider_planning::",
            "selected_dispatch::",
            "target::",
            "native_production::",
        ],
    );
}

#[test]
fn package_review_retains_products_not_checked_compiler_state() {
    assert_absent(
        &[
            include_str!(
                "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
            ),
            include_str!("../../../packages/package-manager/src/review/candidate/passes/policy.rs"),
            include_str!("../../../packages/package-manager/src/review/candidate/review_cache.rs"),
        ],
        "package review interprets",
        &[
            "compiler::CheckedCompilation",
            ".check_package_build(",
            ".execution()",
            ".build_admission()",
            "project_checked_package_policy(",
            "publish_checked_component(",
        ],
    );
    let review = include_str!("checked/package_review.rs");
    assert!(!review.contains("pub fn checked("));
    assert!(!review.contains("Deref"));
}

#[test]
fn entry_custody_has_no_synthetic_service_fields() {
    assert_absent(
        &[
            include_str!("../../realization/provider-planning/src/program_entry.rs"),
            include_str!(
                "../../pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/model.rs"
            ),
            include_str!(
                "../../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal/codec/entry.rs"
            ),
            include_str!("../../backend/artifacts/native-artifact/src/runtime_provisions.rs"),
        ],
        "entry custody retains",
        &[
            "ProgramEntryFusedServiceEstablishment",
            "ProgramEntryReceivingRequirement",
            "fused_service_establishments",
            "fused_service_boundary_slots",
        ],
    );
}

#[test]
fn entry_storage_has_no_provisional_field_mode() {
    for source in [
        include_str!("../../products/src/compile_request/checked.rs"),
        include_str!("../../products/src/package_build.rs"),
        include_str!("checked/package_compilation.rs"),
        include_str!("execution_settlement.rs"),
        include_str!("../../realization/selected-dispatch/src/provider_binding.rs"),
        include_str!("../../realization/selected-dispatch/src/selected_execution.rs"),
        include_str!(
            "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
        ),
        include_str!("../../../packages/package-manager/src/review/candidate/review_cache.rs"),
    ] {
        assert!(!source.contains("unsettled_fused_service_fields"));
        assert!(!source.contains("tolerated_unsettled_service_fields"));
    }
}

#[test]
fn package_binding_policy_does_not_interpret_checked_source() {
    let policy =
        include_str!("../../../packages/package-manager/src/review/candidate/semantic_bindings.rs");
    assert_absent(
        &[policy],
        "binding policy interprets",
        &[
            "CheckedCompilation",
            "provider_planning::",
            ".facts",
            ".symbols",
            ".traits()",
            ".machines()",
            ".providers()",
        ],
    );
    let projection =
        include_str!("../../../packages/review-evidence/src/capture/semantic_bindings.rs");
    assert!(!projection.contains("package_manager"));
    assert!(!projection.contains("PackageKey,"));
    let evidence = include_str!("../../../packages/review-evidence/Cargo.toml");
    assert!(!evidence.contains("package-manager ="));
    assert!(!evidence.contains("compiler ="));
    let packages = include_str!("../../../packages/package-manager/Cargo.toml");
    assert_absent(
        &[packages],
        "package review imports",
        &["provider-planning =", "flow-effects =", "symbols ="],
    );
}

#[test]
fn application_does_not_depend_on_pipeline_stages() {
    let manifest = include_str!("../../../application/cli/Cargo.toml");
    assert!(
        !manifest.contains("/pipeline/"),
        "application operations must call compiler drivers, not numbered transforms"
    );
}

#[test]
fn build_operations_do_not_assemble_compiler_products() {
    assert_absent(
        &[
            include_str!("../../../application/build/src/build.rs"),
            include_str!("../../../application/build/src/project_compilation.rs"),
            include_str!("../../../application/build/src/products.rs"),
        ],
        "Build operation drives",
        &[
            ".admit_trust(",
            "ProductReportPlan",
            "CheckedProductReport::",
            "produce_native_product(",
            "produce_retained_terminal_artifacts(",
            "CheckedCompilation",
            "finish_compilation(",
        ],
    );
}

#[test]
fn accepted_packages_supply_plans_not_compiler_continuations() {
    let source =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    assert_absent(
        &[source],
        "package operation owns",
        &[
            "PreparedPackageBuild",
            ".prepare_build(",
            ".check_build(",
            "with_complete_dependency_build_handoffs(",
            "join_dependency_builds(",
            "with_independent_component_descriptions(",
            "publish_checked_component(",
            "CheckedCompilation",
            "finish_compilation(",
        ],
    );
    let manifest = include_str!("../../../packages/package-manager/Cargo.toml");
    assert!(!manifest.contains("component-description ="));
}

#[test]
fn build_submits_admitted_package_batches_together() {
    let products = include_str!("../../../application/build/src/products.rs");
    assert!(products.contains("crate::project_compilation::compile_returned("));
    let application = include_str!("../../../application/build/src/project_compilation.rs");
    assert!(application.contains("package_manager::operations::admit_project_products("));
    assert!(application.contains("crate::package_compilation::compile_packages("));
    assert!(application.contains(".complete(products, &mut session.packages)"));
    let admission =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    assert_absent(
        &[admission],
        "package admission executes",
        &[
            ".compile_packages(",
            ".check_package(",
            "CompilationSession",
        ],
    );
}

#[test]
fn package_frontiers_batch_components_without_nested_source_workers() {
    let preparation = include_str!("../../../application/build/src/build_execution.rs");
    assert!(preparation.contains("compiler.complete_package_components(builds)"));
    let completion = include_str!("checked/package_compilation.rs");
    assert!(completion.contains("crate::compiler::produce_products(requests)"));
    assert!(completion.contains("ProductRequest::Component"));
    assert!(!preparation.contains("self.check_build("));
    assert!(!preparation.contains("run_on_compile_thread"));
    let batch = include_str!("checked/product_batch.rs");
    assert!(!batch.contains("run_on_compile_thread"));
}

#[test]
fn package_batches_carry_owners_instead_of_parallel_result_tables() {
    for source in [
        include_str!("checked/package_compilation.rs"),
        include_str!("../../../application/build/src/build_execution.rs"),
        include_str!("../../../application/build/src/package_compilation.rs"),
        include_str!("checked/product_batch.rs"),
        include_str!("product_execution/component.rs"),
    ] {
        assert!(!source.contains("map(|_| None)"));
        assert!(!source.contains("outcomes["));
        assert!(!source.contains(".zip("));
    }
}

#[test]
fn terminal_cohorts_retain_admitted_products() {
    let source = include_str!("terminal/terminal_artifact.rs");
    assert!(source.contains("struct AdmittedTerminalProduct"));
    assert_absent(
        &[source],
        "cohort reconstructs",
        &[
            "requests[",
            "entries[",
            "results[",
            "Vec<Vec<usize>>",
            "unwrap_or_else(|_| target::TargetProfile::host())",
        ],
    );
}

#[test]
fn installation_reach_settlement_consumes_prepared_rows() {
    let projection = include_str!(
        "../../realization/provider-planning/src/provider_planning/selected_declarations.rs"
    );
    assert!(projection.contains("provider.row_requirements"));
    assert!(projection.contains("provider.row_realizations"));
    assert!(projection.contains("checked.provider_declarations("));
    let binding = include_str!("../../realization/provider-selection/src/selected_binding.rs");
    assert!(binding.contains("crate::resolve_installation_reach(reaches)"));
    assert!(binding.contains("with_specialized_requirement_rows("));
    assert!(binding.contains("with_external_requirement_applications("));
    assert!(binding.contains("with_installation_reach_resolutions("));
    assert_absent(
        &[binding],
        "selected binding reopens",
        &["CheckFacts", "TypedTrees", "checked_trees", "SymbolHandle"],
    );
    let source = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_declarations.rs"
    );
    assert!(source.contains("capture.conformance_application.trait_scope.scope(typed)"));
    assert!(!source.contains("realized_row_machine("));
    assert_absent(
        &[projection],
        "reach projection reconstructs",
        &[
            "typed.machines()",
            "typed.traits()",
            "attached_data_path",
            "named_provider_row",
            "substitute_nested_installation_reaches",
        ],
    );
    let settlement = include_str!("../../realization/provider-selection/src/installation_reach.rs");
    assert_absent(
        &[settlement],
        "reach settlement reopens",
        &[
            "checked_trees",
            "TypedTrees",
            "SymbolHandle",
            "provider_planning",
        ],
    );
}

#[test]
fn checked_operator_choice_consumes_retained_candidates() {
    let admission = include_str!(
        "../../realization/provider-planning/src/provider_planning/source_provider_selections.rs"
    );
    let dispatch = include_str!(
        "../../realization/selected-dispatch/src/selected_dispatch/operator_adapter.rs"
    );
    for consumer in [admission, dispatch] {
        assert!(consumer.contains("selected_checked_operator_body"));
        assert_absent(
            &[consumer],
            "operator body choice reconstructs",
            &[
                "specialized_provider_machines",
                "machine_specializations",
                "machine_trait_conformances",
                "resolve_satisfied_checked_operator",
                "resolve_specialized_checked_operator",
            ],
        );
    }
    assert!(admission.contains(".project_body_candidates(application,"));
    let projection = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/checked_trees/operators.rs"
    );
    assert!(projection.contains("boundary_declarations"));
    assert!(projection.contains("boundary_body_candidates"));
    assert!(projection.contains(".span(application.body_candidates)"));
    let policy = include_str!("../../realization/provider-selection/src/checked_bodies.rs");
    assert_absent(
        &[policy],
        "checked-body policy reopens",
        &[
            "checked_trees",
            "TypedTrees",
            "SymbolHandle",
            "provider_planning",
        ],
    );
}

#[test]
fn qualification_grants_consume_prepared_issuer_evidence() {
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains(".admitted_qualification_issuers()"));
    let policy = include_str!("../../realization/provider-selection/src/qualification_grants.rs");
    assert_absent(
        &[policy],
        "grant policy reopens",
        &[
            "checked_trees",
            "TypedTrees",
            "SymbolHandle",
            "provider_planning",
        ],
    );
    let preparation = include_str!(
        "../../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/semantic/facts.rs"
    );
    let issuers = preparation
        .find("qualification_evidence::prepare_issuers(")
        .unwrap();
    let occurrences = preparation
        .find("append_contract_semantic_facts(program")
        .unwrap();
    assert!(
        issuers < occurrences,
        "issuer identities must precede occurrence production"
    );
}

#[test]
fn native_provider_handoff_borrows_closed_source_associations() {
    let projection = include_str!("../../products/src/checked_compilation/terminal.rs");
    assert!(projection.contains(".native_provider_sources"));
    assert_absent(
        &[projection],
        "native handoff reopens",
        &[
            ".program.typed",
            "exact_owner_nominal_identity",
            "target_machine_families",
            "machine_by_symbol",
        ],
    );
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("provider_planning::prepare_native_provider_sources("));
}

#[test]
fn selected_intrinsic_settlement_borrows_original_row_declarations() {
    let settlement = include_str!("../../realization/selected-dispatch/src/intrinsic_review.rs");
    assert_absent(
        &[settlement],
        "selected settlement reopens",
        &[
            "CheckedTrees",
            "ProviderPlanningContext",
            "SealedPlacedAccessor::rejoin_plan",
            "row_execution_identity(",
            "accepted_binding_matches_exit_process_row(",
        ],
    );
    assert!(settlement.contains("candidate.row_declarations("));
    assert!(settlement.contains(".direct_declarations("));
    let producer =
        include_str!("../../realization/provider-planning/src/provider_planning/provider_rows.rs");
    assert!(producer.contains("struct ProviderRow {"));
    assert!(producer.contains("direct: Option<DirectBoundaryInput>"));
    assert!(producer.contains("ExitProcessDeclaration::prepare("));
    let review =
        include_str!("../../../packages/review-evidence/src/capture/providers/intrinsics.rs");
    assert!(review.contains("derive_selected_compiler_intrinsic_execution_identity_for_row"));
}

#[test]
fn components_use_the_shared_product_sequence() {
    for source in [
        include_str!("checked/package_compilation.rs"),
        include_str!("checked/package_review.rs"),
    ] {
        assert!(source.contains("ProductRequest::Component"));
        assert!(source.contains("crate::compiler::produce_products("));
        assert!(!source.contains("produce_retained_terminal_artifacts("));
    }
    let component = include_str!("product_execution/component.rs");
    assert!(!component.contains("produce_retained_terminal_artifacts("));
    assert!(!component.contains("TerminalProductionRequest"));
    assert!(component.contains("produced.prepare(request)"));
    assert!(component.contains("accepted_terminal_authority_permission_policy("));
    let (preparation, completion) = component
        .split_once("pub(crate) fn finish(")
        .expect("component preparation precedes completion");
    assert!(preparation.contains("component_calling_applications("));
    assert!(!completion.contains("component_calling_applications("));
    let completion = include_str!("../../native-production/src/application.rs");
    assert!(!completion.contains("program.typed"));
    assert!(!completion.contains("component_calling_applications("));
    assert!(!include_str!("lib.rs").contains("publish_checked_component"));
    assert!(!include_str!("compiler.rs").contains("mod package"));
}

#[test]
fn component_handoff_retains_the_producing_join() {
    let components = include_str!("../../../application/build/src/native_completion.rs");
    assert_absent(
        &[components],
        "native handoff reconstructs",
        &[
            "checked: &",
            ".providers()",
            ".execution()",
            "realizes_selected_plan(",
            "ComponentNativeInputs::join(",
            "identity_digest(",
            ".filter(",
        ],
    );
    assert!(
        components
            .split_whitespace()
            .collect::<String>()
            .contains("component.input()")
    );
    let terminal = include_str!("terminal/terminal_artifact.rs");
    assert!(!terminal.contains("realizes_selected_plan("));
    let selection = include_str!("execution_settlement.rs");
    let initialization = selection
        .split("pub(crate) fn retain_independent_component_products")
        .next()
        .unwrap();
    assert!(initialization.contains("independent_components = Err("));
    assert!(selection.contains("SelectedIndependentComponent::join("));
    let native = include_str!("../../native-production/src/components.rs");
    assert!(native.contains(".admit_native_calls(receiver)"));
}

#[test]
fn terminal_production_owns_the_build_provider_companion() {
    let production = include_str!("terminal/terminal_artifact.rs");
    let completion = include_str!("../../native-production/src/application.rs");
    assert!(completion.contains("pub provider_proposals:"));
    assert!(production.contains("provider_proposals: application.provider_publication.publish("));
    assert!(production.contains("TargetProviderPreparationRequest"));
    assert!(completion.contains("with_target_provider_proposals(self.provider_proposals)"));
    assert!(completion.contains("TerminalNativeProposalRequest<'_>"));
    assert!(completion.contains("if self.physical_entry_adapter"));
    assert_absent(
        &[completion],
        "completion reopens",
        &[
            "produce_target_provider_proposals(",
            "TargetProviderPublicationRequest",
            "program.typed",
            ".provider_defaults()",
            ".providers()",
            "CheckedCompilation",
            "terminal_native_proposal_request(",
        ],
    );
}

#[test]
fn provider_publication_consumes_prepared_declarations_without_source() {
    let preparation = include_str!(
        "../../realization/provider-planning/src/target_machines/provider_publication.rs"
    );
    let publication = include_str!("../../realization/provider-selection/src/publication.rs");
    assert!(!preparation.contains("TerminalArtifactView"));
    assert!(preparation.contains("retain_family_replay("));
    assert!(preparation.contains("canonical_declaration("));
    assert!(!preparation.contains("derive_satisfies_plans"));
    assert!(!preparation.contains("settle_provider_defaults"));
    assert!(!preparation.contains("ProviderSettlement"));
    assert!(
        preparation.contains("provider publication target differs from its authored inventory")
    );
    let inventory = include_str!(
        "../../realization/provider-planning/src/target_machines/authored_providers.rs"
    );
    assert!(inventory.contains("retained.target == Some(profile)"));
    assert!(inventory.contains("return Ok(Arc::clone(retained))"));
    let settlement = include_str!("../../realization/provider-planning/src/provider_settlement.rs");
    assert!(settlement.contains("AuthoredTargetProviders::derive("));
    assert!(!settlement.contains("derive_satisfies_plans"));
    assert_absent(
        &[publication],
        "publication reopens",
        &[
            "ProviderPlanningContext",
            ".typed",
            "derive_satisfies_plans",
            "settle_provider_defaults",
            "exact_owner_nominal_identity",
            "DerivedProviderPlan",
            "SymbolHandle",
            "PreparedSelectionDeclarations",
        ],
    );
    assert!(publication.contains("candidate.explicitly_selected"));
    assert!(publication.contains("module.boundary_machines"));
    assert!(publication.contains("crate::choose("));
    assert!(publication.contains("crate::admit("));
    assert!(publication.contains("TerminalTargetProviderSelection::from_plans"));
    let production = include_str!("terminal/terminal_artifact.rs");
    assert!(production.contains("provider_publication: provider_planning::target_machines::PreparedTargetProviderPublication"));
    let adapter = include_str!(
        "../../realization/provider-planning/src/provider_planning/selection_provenance.rs"
    );
    assert!(adapter.contains("provider_selection::admit("));
    assert!(!adapter.contains("check_operator_family_selections"));
    assert!(!adapter.contains("IndependentComponentJoin"));
}

#[test]
fn provider_derivation_consumes_the_prepared_service_census() {
    let inventory = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_inventory.rs"
    );
    assert!(inventory.contains("PreparedServiceCatalog::prepare("));
    assert!(inventory.contains("self.require_owner(checked)?"));
    let derivation = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/plan_derivation.rs"
    );
    let named = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/named_provider_plans.rs"
    );
    let service_catalog =
        include_str!("../../realization/provider-planning/src/service_declarations.rs");
    for consumer in [derivation, named, service_catalog] {
        for forbidden in [
            "projection::from_typed",
            "service_schema::from_typed",
            "service_schema::from_sealed_placed_accessor",
        ] {
            assert!(
                !consumer.contains(forbidden),
                "provider census consumer reopens {forbidden}"
            );
        }
    }
    assert!(named.contains("services.named_application("));
    assert!(service_catalog.contains("catalog.declared_traits()"));
    let build = include_str!("../../build-evaluation/src/admission/checking.rs");
    assert!(
        build
            .find("PreparedServiceCatalog::prepare(context)")
            .unwrap()
            < build.find("derive_satisfies_plans(").unwrap()
    );
}

#[test]
fn product_coordinator_has_no_native_stage_dependency() {
    let manifest = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .expect("production manifest");
    assert!(!manifest.contains("/pipeline/"));
    assert_absent(
        &[
            include_str!("terminal/terminal_artifact.rs"),
            include_str!("product_execution/native_inputs.rs"),
            include_str!("product_execution.rs"),
            include_str!("product_execution/component.rs"),
        ],
        "coordinator owns",
        &[
            "terminal_psi_to_abstract_operations::",
            "project_terminal_native_realization_proposal(",
            "validate_package_terminal_authority_permission_custody(",
            "terminal_authority_permission_policy_with_rows(",
            "bind_physical_adapter(",
        ],
    );
}

#[test]
fn native_product_supplies_inputs_without_orchestrating_component_emission() {
    let source = include_str!("product_execution.rs");
    let inputs = include_str!("../../../application/build/src/native_completion.rs");
    for operation in [
        "native_production::prepare(&terminal, request)",
        "build_evaluation::scalar_completion::complete(",
        "native_production::compile(prepared)",
    ] {
        assert!(
            !source.contains(operation),
            "compiler still drives {operation}"
        );
        assert!(inputs.contains(operation), "application lost {operation}");
    }
    assert!(source.contains("PendingNativeProduct"));
    assert!(inputs.contains("ComponentCompilation"));
    assert!(inputs.contains("receiving_identities: component.receiving_identities()"));
    assert_absent(
        &[source, inputs],
        "product owner orchestrates",
        &[
            "emit_component_exports(",
            "emit_shared_image(",
            "ComponentNativeCall::join(",
            "with_component_products(",
        ],
    );
}

#[test]
fn product_production_has_no_installed_checkpoint_mode() {
    let compiler = include_str!("compiler.rs");
    assert!(!compiler.contains("PendingRealization"));
    assert!(!compiler.contains("fn finish_compilation("));
    let production = include_str!("terminal/terminal_artifact.rs");
    assert!(!production.contains("enum TerminalProductionView"));
    assert!(!production.contains("TerminalProductionView::Installed"));
    for source in [
        include_str!("checked/compilation_session.rs"),
        include_str!("checked/package_compilation.rs"),
    ] {
        assert!(source.contains("compiler::produce_products("));
    }
}

#[test]
fn product_batch_carries_work_instead_of_empty_outcome_slots() {
    let compiler = include_str!("compiler.rs");
    assert!(compiler.contains("request.plan()"));
    assert!(compiler.contains("produce_retained_terminal_artifacts(terminal_requests)"));
    assert!(!compiler.contains("map(|_| None)"));
    assert!(!compiler.contains("outcomes["));
    assert!(!compiler.contains(".unzip()"));
    assert!(compiler.contains("outcomes.push((owner, outcome))"));
    let execution = include_str!("product_execution.rs");
    assert!(!execution.contains(".settle("));
    assert!(!execution.contains("(settlement, requested)"));
    let reports = include_str!("../../products/src/checked_compilation/report.rs");
    assert_eq!(reports.matches(".settle(").count(), 1);
}

#[test]
fn selected_source_preparation_has_no_post_terminal_installation() {
    let preparation = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(preparation.contains("pub fn prepare_selected_execution("));
    assert!(preparation.contains("retain_selected_compiler_intrinsic_review_identities("));
    assert!(preparation.contains("produce_direct_boundary_leaves("));
    assert!(preparation.contains("resolve_accepted_service_binding("));
    let product = include_str!("product_execution.rs");
    assert!(product.contains("self.realization.into_checked_compilation()"));
    let selection = include_str!("execution_settlement.rs");
    for retired in [
        "CheckedProviderProgram",
        "SelectedExecutionRequest",
        "settle_selected_execution(",
        "install_after_terminal",
    ] {
        assert!(
            !selection.contains(retired),
            "late source phase returned: {retired}"
        );
        assert!(
            !product.contains(retired),
            "late source phase returned: {retired}"
        );
    }
}

#[test]
fn product_installation_does_not_prepare_reference_interpreter_dispatch() {
    let settlement = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert_absent(
        &[settlement],
        "ordinary Product installation retains reference-only work:",
        &[
            "CheckedInterpreterDispatch",
            "prepare_checked_interpreter_dispatch",
            "settle_selected_execution_dispatch",
            "pub dispatch:",
        ],
    );
    let reference =
        include_str!("../../realization/selected-dispatch/src/checked_interpreter_dispatch.rs");
    assert!(reference.contains("settle_selected_boundary_adapter_dispatch("));
    assert!(reference.contains("float_comparisons::selected_executions("));
    let corpus = include_str!("../tests/corpus_runner.rs");
    assert!(corpus.contains("selected_dispatch::prepare_checked_interpreter_dispatch("));
}

#[test]
fn selected_execution_does_not_copy_checked_programs() {
    for source in [
        include_str!("../../realization/selected-dispatch/src/selected_dispatch.rs"),
        include_str!(
            "../../realization/selected-dispatch/src/checked_interpreter_dispatch/boundary_dispatch.rs"
        ),
    ] {
        assert!(!source.contains("Arc::make_mut"));
    }
}

#[test]
fn selected_numeric_admission_consumes_retained_declarations() {
    let evidence = include_str!("../../realization/provider-selection/src/boundary_review.rs");
    assert!(evidence.contains("requirement.numeric_requirement?"));
    assert!(evidence.contains("selected_execution_for_identity("));
    assert!(!evidence.contains(".execution_identity(typed)"));
    assert!(!evidence.contains(".intrinsic_realization_matches(typed,"));
    let preparation = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(preparation.contains("review: &external_providers.boundary_provider_review()"));
    assert!(
        preparation.find(".require_owner(").unwrap()
            < preparation
                .find("bind_selected_provider_plan_facts(")
                .unwrap()
    );
}

#[test]
fn boundary_application_policy_has_one_source_free_owner() {
    let settlement = include_str!(
        "../../pipeline/00_terminal-psi-to-abstract-operations/src/boundary_application_settlement.rs"
    );
    assert!(settlement.contains("provider_selection::select_boundary_applications("));
    assert!(settlement.contains("TerminalBoundaryApplicationCoverage::new("));
    assert!(!settlement.contains("ProviderBinding::"));
    assert!(!settlement.contains("boundary_application_candidates"));
    let source = include_str!("../../realization/provider-selection/src/boundary_review.rs");
    assert!(source.contains("selected_execution_for_identity("));
    assert!(!source.contains("effects::external_providers::selected_numeric_provider("));
}

#[test]
fn selected_boundary_admission_reads_checked_classification() {
    let admission = include_str!("../../realization/provider-selection/src/boundary_review.rs");
    assert_absent(
        &[admission],
        "Product admission reconstructs",
        &[
            "CheckFacts",
            "SymbolHandle",
            "checked_trees",
            "typed:",
            "typed.",
            "IntrinsicRequirement::",
            "schema_binds_exact_",
            "use_site_is_generic_template(",
        ],
    );
    assert!(admission.contains("BoundaryOperatorApplication::GenericTemplate"));
    assert!(admission.contains("crate::select_checked_body(plan,"));
    let projection = include_str!(
        "../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/external_provider_declarations/boundary_review.rs"
    );
    assert!(projection.contains("provider_requirements"));
    assert!(projection.contains("usage.is_generic_template"));
    assert!(projection.contains("operators.project_body_candidates(application,"));
    let adapters = include_str!(
        "../../realization/provider-planning/src/provider_planning/source_provider_selections.rs"
    );
    assert!(!adapters.contains("validate_selected_operator_provider_evidence"));
    assert!(!adapters.contains("validate_selected_requirement_provider_evidence"));
}

#[test]
fn terminal_consumers_call_psi_without_a_product_wrapper() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("terminal-production ="));
    let inspection = include_str!("product_execution/inspection.rs");
    let product = include_str!("terminal/terminal_artifact.rs");
    assert!(product.contains("TerminalProductionRequest"));
    assert!(product.contains(".produce("));
    assert!(!product.contains("produce_terminal_artifact("));
    assert!(inspection.contains("TerminalProductUse::Inspection"));
    assert!(!inspection.contains("TerminalProductionRequest"));
    assert!(!inspection.contains(".produce("));
    let observations = include_str!("observations.rs");
    assert!(observations.contains("PsiCompilationStage::Lowering"));
    assert_eq!(
        observations.matches("record_terminal_production()").count(),
        1
    );
    let policy = include_str!("terminal/behavior_exclusions.rs");
    assert!(policy.contains("impl psi_compiler::TerminalCompositionAdmission"));
}

#[test]
fn exclusion_source_resolution_stays_out_of_terminal_policy() {
    let selection = include_str!("checked/checking/product_selections.rs");
    assert!(selection.contains("source.admit_product_policy("));
    let source =
        include_str!("../../../../psi/compilation/compile-time/src/product_policy/exclusions.rs");
    assert!(source.contains("pub fn resolve("));
    assert!(source.contains("resolve_product_path("));
    let policy = include_str!("terminal/behavior_exclusions.rs");
    assert!(policy.contains("establish_behavior_exclusions("));
    for source_owner in [
        "typed_trees::",
        "checked_trees::",
        "resolve_product_path(",
        "trust_model::",
    ] {
        assert!(
            !policy.contains(source_owner),
            "{source_owner} crossed the Terminal policy boundary"
        );
    }
    let trust = include_str!("../../realization/trust-model/src/lib.rs");
    assert!(!trust.contains("mod behavior_exclusions;"));
    assert!(!trust.contains("harvest_behavior_exclusions"));
    assert!(!trust.contains("verify_entry_behavior_exclusions"));
    let manifest = include_str!("../../realization/trust-model/Cargo.toml");
    for unrelated_owner in [
        "build-time-evaluation =",
        "checked-interpreter =",
        "typed-trees-to-checked-trees =",
        "terminal-installation =",
        "terminal-psi =",
        "target =",
        "semantic-vocabulary =",
    ] {
        assert!(
            !manifest.contains(unrelated_owner),
            "{unrelated_owner} returned to grant reconstruction"
        );
    }
}

#[test]
fn terminal_composition_admission_owns_the_application_root_loop() {
    let producer = include_str!(
        "../../../../psi/compilation/driver/terminal-production/src/terminal_production.rs"
    );
    assert_eq!(producer.matches("application.admit(").count(), 1);
    assert!(!producer.contains("retained_source_machine("));
    let composition = include_str!(
        "../../../../psi/compilation/driver/terminal-production/src/composition_admission.rs"
    );
    assert_eq!(composition.matches("admission.admit(").count(), 1);
    assert!(composition.contains("admission.admit(&lowered.semantic_module, &mut roots)"));
    assert!(!composition.contains("for machine in std::iter::once(selection.machine)"));
    let policy = include_str!("terminal/behavior_exclusions.rs");
    assert_eq!(
        policy
            .matches("close_checked_provider_specializations(")
            .count(),
        1
    );
    assert_eq!(
        policy
            .matches("authored_behavior_exclusion_set_in(")
            .count(),
        1
    );
    let projection = policy
        .find("let first = first?;")
        .expect("first body projection");
    let preparation = policy
        .find("close_checked_provider_specializations(")
        .expect("provider closure");
    let root_loop = policy
        .find("for root in std::iter::once(Ok(first)).chain(roots)")
        .expect("ordered root admission");
    assert!(projection < preparation && preparation < root_loop);
    assert!(policy.contains("&root.provenance"));
    assert!(policy.contains("for root in roots {\n                root?;"));
}

#[test]
fn early_constants_share_terminal_production_without_tree_fallback() {
    let early = include_str!(
        "../../../../psi/compilation/compile-time/src/machine_execution/build_machines.rs"
    );
    let constant = early
        .split("pub(crate) fn evaluate_checked_constant(")
        .nth(1)
        .expect("constant entry")
        .split("fn interpreter_program(")
        .next()
        .expect("constant body");
    assert!(constant.contains("psi_terminal_production::TerminalProductionRequest::new("));
    assert!(constant.contains("terminal_interpreter::TerminalProgram::borrow("));
    assert!(constant.contains(".invoke(meter)"));
    assert!(!constant.contains("checked_interpreter::"));

    let facade = include_str!("../../../../psi/compilation/driver/Cargo.toml");
    let producer =
        include_str!("../../../../psi/compilation/driver/terminal-production/Cargo.toml");
    assert!(facade.contains("psi-terminal-production ="));
    assert!(!producer.contains("build-time-evaluation ="));
    for stage in [
        "checked-trees-to-lowered-psi =",
        "lowered-psi-to-lowered-psi =",
        "lowered-psi-to-terminal-psi =",
    ] {
        assert!(!facade.contains(stage), "source facade reassembles {stage}");
        assert!(producer.contains(stage), "shared producer lost {stage}");
    }
}

#[test]
fn product_consumers_do_not_assemble_source_stages() {
    assert_absent(
        &[
            include_str!("../Cargo.toml"),
            include_str!("../../products/Cargo.toml"),
            include_str!("../../build-evaluation/Cargo.toml"),
        ],
        "product consumer imports",
        &[
            "02_syntax-trees-to-symbol-resolved-trees",
            "03_symbol-resolved-trees-to-typed-trees",
            "03b_typed-trees-to-typed-trees",
            "04_typed-trees-to-checked-trees",
        ],
    );
    assert_absent(
        &[
            include_str!("checked/checking/source_checking.rs"),
            include_str!("../../build-evaluation/src/admission/checking.rs"),
        ],
        "consumer assembles",
        &[
            "prepare_typed_trees",
            "reuse_retained_check",
            "lower_typed_trees_reusing",
            "prepare_source_completion",
            "infer_synchronous_invocations",
        ],
    );
    let product_checking = include_str!("checked/checking/source_checking.rs");
    assert!(!product_checking.contains("provider_planning::"));
    assert!(!product_checking.contains("retain_provider_defaults("));
    let source = include_str!("checked/checking/build_continuation.rs");
    assert_absent(
        &[source],
        "product coordinator assembles",
        &[
            "complete_machine_bodies",
            "join_prepared_allocations",
            "resolve_extension",
            "evaluate_pre_resolution",
        ],
    );
}

#[test]
fn package_source_consumers_do_not_assemble_parser_stages() {
    assert_absent(
        &[
            include_str!("../../../packages/compilation-inputs/Cargo.toml"),
            include_str!("../../../packages/build-declarations/Cargo.toml"),
            include_str!("../../../packages/package-manager/Cargo.toml"),
        ],
        "package consumer imports",
        &["00_source-files-to-tokens", "01_tokens-to-syntax-trees"],
    );
    let assembly = include_str!("../../../packages/compilation-inputs/src/source_assembly.rs");
    assert_absent(
        &[assembly],
        "package assembly owns",
        &[
            "lex_sources(",
            "parse_sources(",
            "parse_source_pages_from(",
            ".adopt_page(",
        ],
    );
}

#[test]
fn packages_do_not_manage_frontend_or_judgment_reuse() {
    assert_absent(
        &[
            include_str!(
                "../../../packages/package-manager/src/operations/accepted_compilation.rs"
            ),
            include_str!(
                "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
            ),
            include_str!("../../../packages/package-manager/src/build_session.rs"),
            include_str!("../../../packages/package-manager/src/build_session/bindings.rs"),
        ],
        "package caller owns",
        &[
            "PreparedCheckedSource",
            "PureBuildCompilation",
            "RetainedCompilationChecks",
            "SettledCheckReuse",
            "compile_checked_reusing_builds",
            "prepared_source",
        ],
    );
}
#[test]
fn executable_callback_placement_is_owned_by_terminal_settlement() {
    let selection = include_str!("../../realization/selected-dispatch/src/selected_execution.rs");
    let source = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/callback_bindings.rs"
    );
    let request = include_str!(
        "../../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal/proposal.rs"
    );
    let persisted = include_str!(
        "../../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal/codec.rs"
    );
    assert!(selection.contains("callback_roots"));
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("provider_selection::select_callback_declarations("));
    for source in [selection, source, persisted] {
        assert!(!source.contains("NativeCallbackPlacement"));
        assert!(!source.contains("SelectedCallbackPlacement"));
    }
    assert!(!request.contains("pub callback_placements:"));
    assert!(request.contains("callback_settlement::place_callbacks"));
}

#[test]
fn callback_root_policy_consumes_prepared_source_correspondence() {
    let preparation = include_str!("../../realization/provider-planning/src/provider_instances.rs");
    assert!(preparation.contains("pub fn callback_providers"));
    assert!(preparation.contains("self.project_instances()?"));
    assert!(!preparation.contains("program.callback_declaration(usage)"));
    assert!(!preparation.contains("program.facts.nominal_machine_uses"));
    assert_eq!(
        preparation
            .matches("exact_checked_adapter(context, plan, row)")
            .count(),
        1
    );
    assert!(preparation.contains("machine: exact_checked_adapter(context, plan, row)"));
    assert!(preparation.contains("adapters.machine(plan, row)"));
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("realization.boundary_arguments.is_empty()"));
    assert!(binding.contains("external_providers.callbacks.calling_requirements("));
    let policy = include_str!("../../realization/provider-selection/src/callback_roots.rs");
    assert!(policy.contains("declarations.declaration(usage)"));
    assert!(policy.contains("self.selected.as_ref()?.selected_requirement_row(identity)"));
    assert!(policy.contains("prepared.plan == plan && prepared.row == row"));
    assert!(policy.contains("providers.selected.is_none()"));
    assert!(!policy.contains("CallbackRootCandidate"));
    assert_absent(
        &[policy],
        "callback root policy reopens",
        &[
            "TypedTrees",
            "CheckFacts",
            "SymbolHandle",
            "provider_planning",
            "checked_trees",
        ],
    );
    let source = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/callback_declarations.rs"
    );
    assert_absent(
        &[source],
        "callback source facts decide",
        &[
            "ProviderPlan",
            "selected_requirement_row",
            "calling_plan_matches",
            "realized_row_machine",
        ],
    );
}

#[test]
fn provider_instance_policy_consumes_checked_declaration_groups() {
    let preparation = include_str!("../../realization/provider-planning/src/provider_instances.rs");
    assert!(
        preparation
            .contains("provider_selection::project_provider_instances(&self.plans, &declarations)")
    );
    assert!(preparation.contains("checked_trees::provider_instances::inherited_instances("));
    assert!(preparation.contains("checked_trees::provider_instances::generic_instances("));
    assert!(!preparation.contains("machine_specializations"));
    assert!(!preparation.contains("provider_closed_requirements"));
    let policy = include_str!("../../realization/provider-selection/src/provider_instances.rs");
    assert!(policy.contains("application.candidates.as_slice()"));
    assert!(policy.contains("declaration.instances.generic_templates.as_slice()"));
    assert!(policy.contains("template.required_instances"));
    assert!(policy.contains("with_specialized_requirement_rows"));
    assert_absent(
        &[policy],
        "provider instance policy reopens",
        &[
            "checked_trees",
            "TypedTrees",
            "CheckFacts",
            "SymbolHandle",
            "provider_planning",
        ],
    );
}

#[test]
fn provider_instance_consumers_share_one_immutable_declaration_owner() {
    let preparation = include_str!("../../realization/provider-planning/src/provider_instances.rs");
    assert!(preparation.contains("pub struct SelectedProviderSource"));
    assert!(preparation.contains("declarations: OnceLock<PreparedProviderInstances>"));
    assert!(preparation.contains("self.declarations.get_or_init("));
    assert!(!preparation.contains("pub fn project_checked_provider_instances"));
    let consumers = [
        include_str!("execution_settlement.rs"),
        include_str!("../../realization/selected-dispatch/src/checked_interpreter_dispatch.rs"),
        include_str!("../../../packages/review-evidence/src/capture/providers/policy.rs"),
    ];
    for consumer in consumers {
        assert!(consumer.contains("provider_source.project_instances()?"));
        assert!(!consumer.contains("provider_planning::project_checked_provider_instances("));
    }
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("provider_source.callback_providers("));
    let execution = include_str!("../../realization/selected-dispatch/src/selected_execution.rs");
    assert!(
        execution.contains("pub provider_source: Arc<provider_planning::SelectedProviderSource>")
    );
    assert!(!execution.contains("pub program:"));
    assert!(!execution.contains("pub selected_provider_plan_facts:"));
}

#[test]
fn provider_demand_is_prepared_by_psi_not_each_target_settlement() {
    let inventory = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_inventory.rs"
    );
    assert_present(
        &[inventory],
        "lost provider demand source",
        &[
            "collect_program_references(",
            ".boundary_applications",
            ".symbolic_boundary_applications",
            ".provider_closed_requirements",
        ],
    );
    assert!(inventory.contains("uses: OnceLock<HashSet<SymbolHandle>>"));
    assert!(inventory.contains("self.require_owner(checked)?"));
    assert!(inventory.contains("std::ptr::eq(self.checked.as_ref(), checked)"));
    let settlement = include_str!("../../realization/provider-planning/src/provider_settlement.rs");
    assert!(settlement.contains("provider_selection::is_candidate_demanded("));
    assert!(!settlement.contains("collect_program_references("));
    assert!(!settlement.contains("provider_closed_requirements"));
    let demand = include_str!("../../realization/provider-selection/src/provider_demands.rs");
    for forbidden in ["TypedTrees", "checked_trees", "CheckFacts", "SymbolHandle"] {
        assert!(!demand.contains(forbidden));
    }
    assert!(demand.contains("choice.subject.selects_plan(schema, plan)"));
    let publication = include_str!(
        "../../realization/provider-planning/src/target_machines/provider_publication.rs"
    );
    assert!(publication.contains("choice.subject.selects_plan(&Ok(schema), &candidate.plan)"));
    assert!(!publication.contains("is_candidate_demanded"));
}

#[test]
fn target_defaults_consume_one_psi_authored_origin_inventory() {
    let target_selection =
        include_str!("../../realization/provider-planning/src/target_machines.rs");
    let selection = target_selection
        .split("pub fn settle_provider_defaults(")
        .nth(1)
        .expect("target-default selection")
        .split("///")
        .next()
        .expect("target-default body");
    assert!(selection.contains("origins: &[crate::SelectedTargetMachineOrigin]"));
    assert!(!selection.contains(".machines()"));
    assert!(!selection.contains("ProviderPlanningContext"));
    let origins = include_str!(
        "../../../../psi/compilation/representations/typed-trees/src/provider_declarations.rs"
    );
    assert!(origins.contains("symbol_is_in_dependency_scope(machine.symbol, scope)"));
    assert!(
        origins.contains("scope == source::DependencyScope::Product || machine.target.is_none()")
    );
    assert!(!origins.contains("TargetProfile"));
    let build = include_str!("../../build-evaluation/src/admission/checking.rs");
    assert!(build.contains("typed_trees::provider_declarations::provider_target_declarations("));
    let inventory = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_inventory.rs"
    );
    assert!(
        inventory.contains("typed_trees::provider_declarations::provider_target_declarations(")
    );
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("Arc::ptr_eq(settlement.provider_inventory.program(), &program)"));
    let production = include_str!("terminal/terminal_artifact.rs");
    assert!(production.contains("origins: source.provider_inventory.origins()"));
    let publication = include_str!(
        "../../realization/provider-planning/src/target_machines/authored_providers.rs"
    );
    assert!(
        publication
            .contains("super::settle_provider_defaults(self.origins, self.defaults, profile)")
    );
}

#[test]
fn target_body_policy_consumes_psi_direct_call_declarations() {
    let projection =
        include_str!("../../../../psi/compilation/representations/typed-trees/src/direct_calls.rs");
    assert_present(
        &[projection],
        "missing direct-call declaration",
        &[
            "ExplicitStateCallees",
            "called_machine_position(typed,",
            "machine_authored_target(machine)",
            "source_target_context(machine)",
            "target: machine",
            "symbol_is_in_dependency_scope",
        ],
    );
    assert!(!projection.contains("TargetProfile"));
    let policy = include_str!("../../realization/provider-selection/src/target_bodies.rs");
    assert_absent(
        &[policy],
        "target-body policy reopens",
        &[
            "TypedTrees",
            "checked_trees",
            "SymbolHandle",
            "ExplicitStateCallees",
            "typed.",
        ],
    );
    assert!(policy.contains("fn explicit_body"));
    assert!(policy.contains("fn realizes_provider_body"));
    let bridge = include_str!("../../realization/provider-planning/src/target_machines.rs");
    assert!(!bridge.contains("ExplicitStateCallees"));
    assert!(!bridge.contains(".machine_states("));
    assert!(!bridge.contains("fn called_machine("));
    assert!(bridge.contains("provider_selection::reject_reached_inert_calls("));
    let derivation = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/plan_derivation.rs"
    );
    assert!(derivation.contains("provider_selection::DirectTargetBodyAbsences::derive("));
    let inventory = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_inventory.rs"
    );
    assert!(inventory.contains("direct_calls: OnceLock<"));
    assert!(inventory.contains("self.require_owner(checked)?"));
    assert!(inventory.contains("typed_trees::direct_calls::collect("));
    let build = include_str!("../../build-evaluation/src/admission/checking.rs");
    assert!(build.contains("typed_trees::direct_calls::collect(context.typed, context.scope)"));
}

#[test]
fn selected_callback_custody_carries_declarations_not_source_symbols() {
    let selection = include_str!("../../realization/selected-dispatch/src/selected_execution.rs");
    assert!(
        selection.contains("pub callback_roots: psi_terminal_production::PreparedTerminalRoots")
    );
    let custody =
        include_str!("../../../../psi/compilation/driver/terminal-production/src/custody.rs");
    assert!(custody.contains("pub callback_roots: Option<&'a crate::PreparedTerminalRoots>"));
    assert!(!custody.contains("pub admission_roots: &'a [symbols::SymbolHandle]"));
    let preparation = include_str!(
        "../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/prepared_terminal_roots.rs"
    );
    assert!(!preparation.contains("symbols::SymbolHandle"));
    assert!(preparation.contains("declaration.same_identity_as("));
    let declarations = include_str!(
        "../../../../psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/external_provider_declarations/callbacks.rs"
    );
    assert!(declarations.contains("checked.callback_declaration(usage)"));
    assert!(declarations.contains("declarations::identity("));
    assert!(declarations.contains("RootDeclaration::Unavailable(reason.clone())"));
    assert!(declarations.contains("usage.published_requirement_envelope"));
    assert!(declarations.contains("usage.selected_actual_envelope"));
    assert!(declarations.contains("usage.callback_entry.map("));
    let admission = include_str!(
        "../../../../psi/compilation/driver/terminal-production/src/composition_admission.rs"
    );
    assert!(admission.contains("entry.chain(callbacks).chain(adapter)"));
    assert!(admission.contains("roots.retained_roots(&lowered.semantic_module)"));
}

#[test]
fn service_surface_projection_is_psi_owned_and_calling_is_an_omega_attachment() {
    let surface =
        include_str!("../../../../psi/foundation/language-semantics/src/service_surface.rs");
    assert!(!surface.contains("BoundaryCallingPlanCommitment"));
    assert!(!surface.contains("calling_plan_report_fingerprint"));
    let projection = include_str!(
        "../../../../psi/compilation/representations/typed-trees/src/service_declarations/projection.rs"
    );
    assert!(projection.contains("requirements.push(ServiceRequirementSource"));
    assert!(projection.contains("scope.type_identity("));
    assert!(!projection.contains("BoundaryCallingPlanRealization"));
    let attachment = include_str!("../../realization/provider-planning/src/service_schema/mod.rs");
    assert!(attachment.contains("application.argument_owner() == argument_owner"));
    assert!(attachment.contains("application.boundary_arguments == boundary_arguments"));
    assert!(attachment.contains("prepared.argument_owner,"));
    assert!(attachment.contains("prepared.boundary_arguments,"));
    assert!(!attachment.contains("ServiceMethod {"));
    assert!(!attachment.contains("fn collect_service_methods("));
    let catalog = include_str!("../../realization/provider-planning/src/service_declarations.rs");
    assert!(catalog.contains("projected.exact_requirements()?"));
    assert!(!catalog.contains("trait_requirements("));
}
