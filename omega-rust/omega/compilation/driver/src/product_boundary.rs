//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

#[test]
fn final_representation_admission_belongs_to_psi_checking() {
    let coordinator = include_str!("checked/checking/source_checking.rs");
    assert!(coordinator.contains("psi_compiler::ProductRepresentationInput"));
    for forbidden in [
        "rederive_opaque_representation_selections(",
        "OpaqueDataPropertyReceipt::copy(",
        "CheckedSemanticCopy",
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
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
    for forbidden in [
        "source.typed(",
        "mod exclusions",
        "collect_product_root_grants(",
        "collect_opaque_representation_selections(",
        "native_bindings::collect",
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
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
    for forbidden in [
        "ComputedBuildConfig",
        "package_compilation::",
        "effects::",
        "provider_planning::",
    ] {
        assert!(!source.contains(forbidden), "Psi owns {forbidden}");
    }
}

#[test]
fn product_declaration_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert!(coordinator.contains("source.admit_product_policy("));
    for forbidden in [
        "mod additional_declarations",
        "resolve_product_path(",
        "WireCompatibilityFact",
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
    let admission =
        include_str!("../../../../psi/compilation/compile-time/src/product_declarations.rs");
    for required in [
        "request.validate_shape()",
        "resolve_build_request_author(",
        "symbol_package_identity(root)",
        "declared.contains(request)",
    ] {
        assert!(
            admission.contains(required),
            "Psi admission lost {required}"
        );
    }
    for forbidden in ["ComputedBuildConfig", "provider_planning::", "effects::"] {
        assert!(
            !admission.contains(forbidden),
            "Psi admission depends on {forbidden}"
        );
    }
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
    for retired in [
        "ProductEntryQueryCompatibility",
        "fn core_arrival_requirement_contract(",
        "fn arrival_requirement_contract(",
    ] {
        assert!(
            !realization.contains(retired),
            "Omega owns source judgment {retired}"
        );
    }
    let catalog = include_str!("../../backend/target/src/target_profile.rs");
    assert!(catalog.contains("ProgramEntrySourceCatalog for ProgramEntrySlots"));
    for source_type in [
        "TypedTrees",
        "ProductEntryDeclaration",
        "NormalizedTypeIdentity",
    ] {
        assert!(
            !catalog.contains(source_type),
            "target catalog receives {source_type}"
        );
    }
}

#[test]
fn product_provider_request_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert!(coordinator.contains("source.admit_product_policy("));
    for forbidden in [
        "prepare_provider_applications(",
        "prepare_product_operand_application(",
        "collect_product_provider_selections(",
        "collect_product_selection_occurrences(",
        "resolve_build_request_author(",
        "ProductApplicationDemand",
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
    let continuation = include_str!("../../../../psi/compilation/driver/src/source_compilation.rs");
    assert!(!continuation.contains("pub fn prepare_product_operand_application("));
    let admission = include_str!(
        "../../../../psi/compilation/driver/src/source_compilation/provider_requests.rs"
    );
    assert!(admission.contains("pub(super) fn admit_product_provider_requests("));
    assert!(admission.contains("collect_product_provider_selections("));
    for forbidden in [
        "ComputedBuildConfig",
        "provider_planning::",
        "provider_selection::",
    ] {
        assert!(
            !admission.contains(forbidden),
            "Psi admission owns {forbidden}"
        );
    }
}

#[test]
fn selected_constant_evaluation_belongs_to_psi() {
    let selection = include_str!("../../realization/selected-dispatch/src/constant_evaluation.rs");
    assert!(selection.contains("build_time_evaluation::SelectedConstantEvaluationRequest"));
    for forbidden in [
        "evaluate_selected_operators(",
        ".finish(typed)",
        "fold_data_layout_queries(",
    ] {
        assert!(
            !selection.contains(forbidden),
            "selection drives {forbidden}"
        );
    }
    let geometry = include_str!("../../realization/provider-planning/src/data_layout_queries.rs");
    for forbidden in [
        "expression_mut(",
        "proof_only::",
        "PreCheckEvaluation",
        "reject_unresolved_layout_operands",
    ] {
        assert!(!geometry.contains(forbidden), "geometry owns {forbidden}");
    }
    let evaluation = include_str!(
        "../../../../psi/compilation/compile-time/src/const_evaluation/selected_constants.rs"
    );
    let queries = include_str!(
        "../../../../psi/compilation/compile-time/src/const_evaluation/data_layout_queries.rs"
    );
    for source in [evaluation, queries] {
        for forbidden in [
            "provider_planning::",
            "selected_dispatch::",
            "target::",
            "layout::",
        ] {
            assert!(
                !source.contains(forbidden),
                "Psi evaluation depends on {forbidden}"
            );
        }
    }
}

#[test]
fn deferred_constants_use_canonical_entries_without_source_snapshots() {
    let checking = include_str!("checked/checking/source_checking.rs");
    let closure = include_str!("../../build-evaluation/src/scalar_completion.rs");
    let product = include_str!("../../products/src/prepared_build.rs");
    for source in [checking, closure, product] {
        for forbidden in [
            "constant_recipe_source",
            "constant_recipe_continuations",
            "read_constant_recipe_closure(",
            "with_constant_recipe_closure(",
            "plan_laid_layouts =",
            "const_declarations.for_each_mut",
        ] {
            assert!(!source.contains(forbidden), "coordinator owns {forbidden}");
        }
    }
    assert!(closure.contains("build_time_evaluation::complete_scalar_applications("));
    let checking = include_str!("../../../../psi/compilation/driver/src/source_checking.rs");
    assert!(checking.contains("constant_entries::retain_evaluation_entries(&mut program)"));
    let selection = include_str!("../../realization/selected-dispatch/src/constant_evaluation.rs");
    for source in [checking, product, selection] {
        for retired in [
            "RetainedConstantEvaluation",
            "ConstantValueClosureRequest",
            "close_constants(",
        ] {
            assert!(
                !source.contains(retired),
                "source snapshot path returned: {retired}"
            );
        }
    }
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
    for forbidden in [
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
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
    let facade = include_str!("../../../../psi/compilation/driver/src/lib.rs");
    let stage = include_str!(
        "../../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/lib.rs"
    );
    for source in [facade, stage] {
        for retired in [
            "ConstantProofClosure",
            "ConstantProofApplication",
            "settle_closed_constant_execution",
            "pub use facts::declaration_descriptions::retain_declaration_descriptions",
        ] {
            assert!(
                !source.contains(retired),
                "source rewrite API returned: {retired}"
            );
        }
    }
    let entries =
        include_str!("../../../../psi/compilation/driver/src/source_checking/constant_entries.rs");
    assert!(entries.contains("fn retain_evaluation_entries("));
    assert!(!entries.contains("CheckedTrees::with_roots("));
    let applications =
        include_str!("../../../../psi/foundation/semantic-vocabulary/src/proof_applications.rs");
    for forbidden in [
        "provider_planning::",
        "selected_dispatch::",
        "target::",
        "native_production::",
    ] {
        assert!(
            !entries.contains(forbidden) && !applications.contains(forbidden),
            "Psi constant entries or proof applications depend on {forbidden}"
        );
    }
}

#[test]
fn package_review_retains_products_not_checked_compiler_state() {
    for source in [
        include_str!(
            "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
        ),
        include_str!("../../../packages/package-manager/src/review/candidate/passes/policy.rs"),
        include_str!("../../../packages/package-manager/src/review/candidate/review_cache.rs"),
    ] {
        for forbidden in [
            "compiler::CheckedCompilation",
            ".check_package_build(",
            ".execution()",
            ".build_admission()",
            "project_checked_package_policy(",
            "publish_checked_component(",
        ] {
            assert!(
                !source.contains(forbidden),
                "package review interprets {forbidden}"
            );
        }
    }
    let review = include_str!("checked/package_review.rs");
    assert!(!review.contains("pub fn checked("));
    assert!(!review.contains("Deref"));
}

#[test]
fn entry_custody_has_no_synthetic_service_fields() {
    for source in [
        include_str!("../../realization/provider-planning/src/program_entry.rs"),
        include_str!(
            "../../pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement/model.rs"
        ),
        include_str!(
            "../../pipeline/00_terminal-psi-to-abstract-operations/src/retained_terminal/codec/entry.rs"
        ),
        include_str!("../../backend/artifacts/native-artifact/src/runtime_provisions.rs"),
    ] {
        for retired in [
            "ProgramEntryFusedServiceEstablishment",
            "ProgramEntryReceivingRequirement",
            "fused_service_establishments",
            "fused_service_boundary_slots",
        ] {
            assert!(!source.contains(retired), "entry custody retains {retired}");
        }
    }
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
    for forbidden in [
        "CheckedCompilation",
        "provider_planning::",
        ".facts",
        ".symbols",
        ".traits()",
        ".machines()",
        ".providers()",
    ] {
        assert!(
            !policy.contains(forbidden),
            "binding policy interprets {forbidden}"
        );
    }
    let projection =
        include_str!("../../../packages/review-evidence/src/capture/semantic_bindings.rs");
    assert!(!projection.contains("package_manager"));
    assert!(!projection.contains("PackageKey,"));
    let evidence = include_str!("../../../packages/review-evidence/Cargo.toml");
    assert!(!evidence.contains("package-manager ="));
    assert!(!evidence.contains("compiler ="));
    let packages = include_str!("../../../packages/package-manager/Cargo.toml");
    for dependency in ["provider-planning =", "flow-effects =", "symbols ="] {
        assert!(
            !packages.contains(dependency),
            "package review imports {dependency}"
        );
    }
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
    for source in [
        include_str!("../../../application/build/src/build.rs"),
        include_str!("../../../application/build/src/project_compilation.rs"),
        include_str!("../../../application/build/src/products.rs"),
    ] {
        for forbidden in [
            ".admit_trust(",
            "ProductReportPlan",
            "CheckedProductReport::",
            "produce_native_product(",
            "produce_retained_terminal_artifacts(",
            "CheckedCompilation",
            "finish_compilation(",
        ] {
            assert!(
                !source.contains(forbidden),
                "Build operation drives {forbidden}"
            );
        }
    }
}

#[test]
fn accepted_packages_supply_plans_not_compiler_continuations() {
    let source =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    for forbidden in [
        "PreparedPackageBuild",
        ".prepare_build(",
        ".check_build(",
        "with_complete_dependency_build_handoffs(",
        "join_dependency_builds(",
        "with_independent_component_descriptions(",
        "publish_checked_component(",
        "CheckedCompilation",
        "finish_compilation(",
    ] {
        assert!(
            !source.contains(forbidden),
            "package operation owns {forbidden}"
        );
    }
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
    for forbidden in [
        ".compile_packages(",
        ".check_package(",
        "CompilationSession",
    ] {
        assert!(
            !admission.contains(forbidden),
            "package admission executes {forbidden}"
        );
    }
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
    for forbidden in [
        "requests[",
        "entries[",
        "results[",
        "Vec<Vec<usize>>",
        "unwrap_or_else(|_| target::TargetProfile::host())",
    ] {
        assert!(
            !source.contains(forbidden),
            "cohort reconstructs {forbidden}"
        );
    }
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
    for forbidden in ["CheckFacts", "TypedTrees", "checked_trees", "SymbolHandle"] {
        assert!(
            !binding.contains(forbidden),
            "selected binding reopens {forbidden}"
        );
    }
    let source = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/provider_declarations.rs"
    );
    assert!(source.contains("capture.conformance_application.trait_scope.scope(typed)"));
    assert!(!source.contains("realized_row_machine("));
    for forbidden in [
        "typed.machines()",
        "typed.traits()",
        "attached_data_path",
        "named_provider_row",
        "substitute_nested_installation_reaches",
    ] {
        assert!(
            !projection.contains(forbidden),
            "reach projection reconstructs {forbidden}"
        );
    }
    let settlement = include_str!("../../realization/provider-selection/src/installation_reach.rs");
    for forbidden in [
        "checked_trees",
        "TypedTrees",
        "SymbolHandle",
        "provider_planning",
    ] {
        assert!(
            !settlement.contains(forbidden),
            "reach settlement reopens {forbidden}"
        );
    }
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
        for forbidden in [
            "specialized_provider_machines",
            "machine_specializations",
            "machine_trait_conformances",
            "resolve_satisfied_checked_operator",
            "resolve_specialized_checked_operator",
        ] {
            assert!(
                !consumer.contains(forbidden),
                "operator body choice reconstructs {forbidden}"
            );
        }
    }
    assert!(admission.contains(".project_body_candidates(application,"));
    let projection = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/checked_trees/operators.rs"
    );
    assert!(projection.contains("boundary_declarations"));
    assert!(projection.contains("boundary_body_candidates"));
    assert!(projection.contains(".span(application.body_candidates)"));
    let policy = include_str!("../../realization/provider-selection/src/checked_bodies.rs");
    for forbidden in [
        "checked_trees",
        "TypedTrees",
        "SymbolHandle",
        "provider_planning",
    ] {
        assert!(
            !policy.contains(forbidden),
            "checked-body policy reopens {forbidden}"
        );
    }
}

#[test]
fn qualification_grants_consume_prepared_issuer_evidence() {
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains(".admitted_qualification_issuers()"));
    let policy = include_str!("../../realization/provider-selection/src/qualification_grants.rs");
    for forbidden in [
        "checked_trees",
        "TypedTrees",
        "SymbolHandle",
        "provider_planning",
    ] {
        assert!(
            !policy.contains(forbidden),
            "grant policy reopens {forbidden}"
        );
    }
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
    for forbidden in [
        ".program.typed",
        "exact_owner_nominal_identity",
        "target_machine_families",
        "machine_by_symbol",
    ] {
        assert!(
            !projection.contains(forbidden),
            "native handoff reopens {forbidden}"
        );
    }
    let binding = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert!(binding.contains("provider_planning::prepare_native_provider_sources("));
}

#[test]
fn selected_intrinsic_settlement_borrows_original_row_declarations() {
    let settlement = include_str!("../../realization/selected-dispatch/src/intrinsic_review.rs");
    for forbidden in [
        "CheckedTrees",
        "ProviderPlanningContext",
        "SealedPlacedAccessor::rejoin_plan",
        "row_execution_identity(",
        "accepted_binding_matches_exit_process_row(",
    ] {
        assert!(
            !settlement.contains(forbidden),
            "selected settlement reopens {forbidden}"
        );
    }
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
    for forbidden in [
        "checked: &",
        ".providers()",
        ".execution()",
        "realizes_selected_plan(",
        "ComponentNativeInputs::join(",
        "identity_digest(",
        ".filter(",
    ] {
        assert!(
            !components.contains(forbidden),
            "native handoff reconstructs {forbidden}"
        );
    }
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
    for forbidden in [
        "produce_target_provider_proposals(",
        "TargetProviderPublicationRequest",
        "program.typed",
        ".provider_defaults()",
        ".providers()",
        "CheckedCompilation",
        "terminal_native_proposal_request(",
    ] {
        assert!(
            !completion.contains(forbidden),
            "completion reopens {forbidden}"
        );
    }
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
    for forbidden in [
        "ProviderPlanningContext",
        ".typed",
        "derive_satisfies_plans",
        "settle_provider_defaults",
        "exact_owner_nominal_identity",
        "DerivedProviderPlan",
        "SymbolHandle",
        "PreparedSelectionDeclarations",
    ] {
        assert!(
            !publication.contains(forbidden),
            "publication reopens {forbidden}"
        );
    }
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
fn product_coordinator_has_no_native_stage_dependency() {
    let manifest = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .expect("production manifest");
    assert!(!manifest.contains("/pipeline/"));
    for source in [
        include_str!("terminal/terminal_artifact.rs"),
        include_str!("product_execution/native_inputs.rs"),
        include_str!("product_execution.rs"),
        include_str!("product_execution/component.rs"),
    ] {
        for stage_operation in [
            "terminal_psi_to_abstract_operations::",
            "project_terminal_native_realization_proposal(",
            "validate_package_terminal_authority_permission_custody(",
            "terminal_authority_permission_policy_with_rows(",
            "bind_physical_adapter(",
        ] {
            assert!(
                !source.contains(stage_operation),
                "coordinator owns {stage_operation}"
            );
        }
    }
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
    for source in [source, inputs] {
        for forbidden in [
            "emit_component_exports(",
            "emit_shared_image(",
            "ComponentNativeCall::join(",
            "with_component_products(",
        ] {
            assert!(
                !source.contains(forbidden),
                "product owner orchestrates {forbidden}"
            );
        }
    }
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
    for reference_work in [
        "CheckedInterpreterDispatch",
        "prepare_checked_interpreter_dispatch",
        "settle_selected_execution_dispatch",
        "pub dispatch:",
    ] {
        assert!(
            !settlement.contains(reference_work),
            "ordinary Product installation retains reference-only work: {reference_work}"
        );
    }
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
    for forbidden in [
        "CheckFacts",
        "SymbolHandle",
        "checked_trees",
        "typed:",
        "typed.",
        "IntrinsicRequirement::",
        "schema_binds_exact_",
        "use_site_is_generic_template(",
    ] {
        assert!(
            !admission.contains(forbidden),
            "Product admission reconstructs {forbidden}"
        );
    }
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
    for manifest in [
        include_str!("../Cargo.toml"),
        include_str!("../../products/Cargo.toml"),
        include_str!("../../build-evaluation/Cargo.toml"),
    ] {
        for stage in [
            "02_syntax-trees-to-symbol-resolved-trees",
            "03_symbol-resolved-trees-to-typed-trees",
            "03b_typed-trees-to-typed-trees",
            "04_typed-trees-to-checked-trees",
        ] {
            assert!(
                !manifest.contains(stage),
                "product consumer imports {stage}"
            );
        }
    }
    for source in [
        include_str!("checked/checking/source_checking.rs"),
        include_str!("../../build-evaluation/src/admission/checking.rs"),
    ] {
        for operation in [
            "prepare_typed_trees",
            "reuse_retained_check",
            "lower_typed_trees_reusing",
            "prepare_source_completion",
            "infer_synchronous_invocations",
        ] {
            assert!(
                !source.contains(operation),
                "consumer assembles {operation}"
            );
        }
    }
    let product_checking = include_str!("checked/checking/source_checking.rs");
    assert!(!product_checking.contains("provider_planning::"));
    assert!(!product_checking.contains("retain_provider_defaults("));
    let source = include_str!("checked/checking/build_continuation.rs");
    for operation in [
        "complete_machine_bodies",
        "join_prepared_allocations",
        "resolve_extension",
        "evaluate_pre_resolution",
    ] {
        assert!(
            !source.contains(operation),
            "product coordinator assembles {operation}"
        );
    }
}

#[test]
fn package_source_consumers_do_not_assemble_parser_stages() {
    for manifest in [
        include_str!("../../../packages/compilation-inputs/Cargo.toml"),
        include_str!("../../../packages/build-declarations/Cargo.toml"),
        include_str!("../../../packages/package-manager/Cargo.toml"),
    ] {
        for stage in ["00_source-files-to-tokens", "01_tokens-to-syntax-trees"] {
            assert!(
                !manifest.contains(stage),
                "package consumer imports {stage}"
            );
        }
    }
    let assembly = include_str!("../../../packages/compilation-inputs/src/source_assembly.rs");
    for operation in [
        "lex_sources(",
        "parse_sources(",
        "parse_source_pages_from(",
        ".adopt_page(",
    ] {
        assert!(
            !assembly.contains(operation),
            "package assembly owns {operation}"
        );
    }
}

#[test]
fn packages_do_not_manage_frontend_or_judgment_reuse() {
    for source in [
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs"),
        include_str!(
            "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
        ),
        include_str!("../../../packages/package-manager/src/build_session.rs"),
        include_str!("../../../packages/package-manager/src/build_session/bindings.rs"),
    ] {
        for forbidden in [
            "PreparedCheckedSource",
            "PureBuildCompilation",
            "RetainedCompilationChecks",
            "SettledCheckReuse",
            "compile_checked_reusing_builds",
            "prepared_source",
        ] {
            assert!(
                !source.contains(forbidden),
                "package caller owns {forbidden}"
            );
        }
    }
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
    let roots = include_str!("../../realization/provider-planning/src/callback_roots.rs");
    assert!(roots.contains("provider_selection::select_callback_roots(candidates)"));
    for source in [selection, source, persisted] {
        assert!(!source.contains("NativeCallbackPlacement"));
        assert!(!source.contains("SelectedCallbackPlacement"));
    }
    assert!(!request.contains("pub callback_placements:"));
    assert!(request.contains("callback_settlement::place_callbacks"));
}

#[test]
fn callback_root_policy_consumes_prepared_source_correspondence() {
    let preparation = include_str!("../../realization/provider-planning/src/callback_roots.rs");
    assert!(preparation.contains("project_checked_provider_instances("));
    assert!(preparation.contains("program.callback_declaration(usage)"));
    assert!(preparation.contains("selected_instances.selected_requirement_row(identity)"));
    assert!(preparation.contains("crate::exact_checked_adapter(context, plan, row)"));
    assert!(preparation.contains("realization.boundary_arguments.is_empty()"));
    let policy = include_str!("../../realization/provider-selection/src/callback_roots.rs");
    for forbidden in [
        "TypedTrees",
        "CheckFacts",
        "SymbolHandle",
        "provider_planning",
        "checked_trees",
    ] {
        assert!(
            !policy.contains(forbidden),
            "callback root policy reopens {forbidden}"
        );
    }
    let source = include_str!(
        "../../../../psi/compilation/representations/checked-trees/src/callback_declarations.rs"
    );
    for forbidden in [
        "ProviderPlan",
        "selected_requirement_row",
        "calling_plan_matches",
        "realized_row_machine",
    ] {
        assert!(
            !source.contains(forbidden),
            "callback source facts decide {forbidden}"
        );
    }
}
