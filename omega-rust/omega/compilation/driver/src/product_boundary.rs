//! Source guards for ownership and ordering rules that have no structural form
//! yet.
//!
//! Dependency direction between owners is not checked here. It lives in
//! `tools/check_native_boundary.py`, which reads `cargo metadata`, so a wrong
//! import stops resolving instead of relying on a spelling. What remains is
//! module discipline inside crates that legitimately depend on the owner
//! (a coordinator must not call an operation Psi owns), plus the order and
//! count of operations in a sequence. Each needs a crate split, a narrower
//! visibility, or an instrumented run before the text search can go.

/// Assert every `source` omits each listed token, naming the owner that must
/// not spell it.
fn assert_absent(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(!source.contains(token), "{complaint} {token}");
        }
    }
}

/// Assert the operations first appear in `source` in the listed order.
fn assert_in_order(source: &str, owner: &str, operations: &[&str]) {
    let mut previous = None;
    for operation in operations {
        let position = source
            .find(operation)
            .unwrap_or_else(|| panic!("{owner} lost {operation}"));
        assert!(
            previous.is_none_or(|before| before < position),
            "{owner} runs {operation} out of order"
        );
        previous = Some(position);
    }
}

#[test]
fn final_representation_admission_belongs_to_psi_checking() {
    let coordinator = include_str!("checked/checking/source_checking.rs");
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
    assert_in_order(
        checking,
        "Psi source checking",
        &[
            "self.admission.representations.admit(&program)",
            "validation::source_completion::prepare_source_completion(",
            "prior.judges_preparation_input(",
        ],
    );
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
    assert_in_order(
        coordinator,
        "product selections",
        &[
            "rejoin_package_provider_selections(",
            "source.admit_product_policy(",
            "native_binding_evaluation",
        ],
    );
    let source =
        include_str!("../../../../psi/compilation/driver/src/source_compilation/product_policy.rs");
    assert_in_order(
        source,
        "Psi product policy",
        &[
            "self.admit_product_provider_requests(",
            "collect_opaque_representation_selections(",
            "native_bindings::collect(",
            "collect_product_root_grants(",
            "product_declarations::admit(",
            "collect_behavior_exclusions(",
        ],
    );
}

#[test]
fn product_declaration_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "mod additional_declarations",
            "resolve_product_path(",
            "WireCompatibilityFact",
        ],
    );
}

#[test]
fn source_entry_admission_never_calls_omega_with_typed_trees() {
    let query = include_str!("checked/checking/product_queries.rs");
    assert_absent(&[query], "coordinator owns", &["provider_planning::"]);
    let completion = include_str!("checked/checking/build_continuation.rs");
    assert_absent(
        &[completion],
        "coordinator owns",
        &["provider_planning::program_entry::collect_root_bindings("],
    );
}

#[test]
fn product_provider_request_admission_belongs_to_psi() {
    let coordinator = include_str!("checked/checking/product_selections.rs");
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
}

#[test]
fn selected_constant_evaluation_belongs_to_psi() {
    let selection = include_str!("../../realization/selected-dispatch/src/constant_evaluation.rs");
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
            ".compile_packages(",
            ".check_package(",
            "CompilationSession",
        ],
    );
}

#[test]
fn package_frontiers_batch_components_without_nested_source_workers() {
    assert_absent(
        &[
            include_str!("../../../application/build/src/build_execution.rs"),
            include_str!("checked/product_batch.rs"),
        ],
        "package batch nests source work:",
        &["run_on_compile_thread"],
    );
    assert_absent(
        &[include_str!(
            "../../../application/build/src/build_execution.rs"
        )],
        "package batch nests source work:",
        &["self.check_build("],
    );
}

#[test]
fn installation_reach_settlement_consumes_prepared_rows() {
    let projection = include_str!(
        "../../realization/provider-planning/src/provider_planning/selected_declarations.rs"
    );
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
}

#[test]
fn checked_operator_choice_consumes_retained_candidates() {
    let admission = include_str!(
        "../../realization/provider-planning/src/provider_planning/source_provider_selections.rs"
    );
    let dispatch = include_str!(
        "../../realization/selected-dispatch/src/selected_dispatch/operator_adapter.rs"
    );
    assert_absent(
        &[admission, dispatch],
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

#[test]
fn issuer_identities_precede_occurrence_production() {
    let preparation = include_str!(
        "../../../../psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/semantic/facts.rs"
    );
    assert_in_order(
        preparation,
        "contract fact preparation",
        &[
            "qualification_evidence::prepare_issuers(",
            "append_contract_semantic_facts(program",
        ],
    );
}

#[test]
fn native_provider_handoff_borrows_closed_source_associations() {
    let projection = include_str!("../../products/src/checked_compilation/terminal.rs");
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
            "SealedAccessor::rejoin_plan",
            "row_execution_identity(",
            "accepted_binding_matches_exit_process_row(",
        ],
    );
}

#[test]
fn components_use_the_shared_product_sequence() {
    assert_absent(
        &[
            include_str!("checked/package_compilation.rs"),
            include_str!("checked/package_review.rs"),
            include_str!("product_execution/component.rs"),
        ],
        "component production bypasses the shared sequence:",
        &["produce_retained_terminal_artifacts("],
    );
    let component = include_str!("product_execution/component.rs");
    assert_absent(
        &[component],
        "component production bypasses the shared sequence:",
        &["TerminalProductionRequest"],
    );
    let (_, completion) = component
        .split_once("pub(crate) fn finish(")
        .expect("component preparation precedes completion");
    assert_absent(
        &[
            completion,
            include_str!("../../native-production/src/application.rs"),
        ],
        "component completion rebuilds source applications:",
        &["component_calling_applications("],
    );
    assert_absent(
        &[include_str!("lib.rs")],
        "coordinator exports",
        &["publish_checked_component"],
    );
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
    let terminal = include_str!("terminal/terminal_artifact.rs");
    assert_absent(
        &[terminal],
        "terminal production rejoins components:",
        &["realizes_selected_plan("],
    );
}

#[test]
fn calling_materialization_consumes_prepared_psi_telescopes() {
    let signatures = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/boundary_signatures.rs"
    );
    let materialization = signatures
        .split("pub(crate) fn materialize_prepared_signature(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn exact_boundary_layout_root_symbol(")
        .next()
        .unwrap();
    assert_absent(
        &[materialization],
        "calling materialization rebuilds source telescope",
        &[
            "state_signature_parameters",
            "state_signature_type_parameters",
            "MachineParameterContract",
            "native_callback_parameters",
            "nominal_callback_native_parameter_id",
            "callback_nominal_report_fingerprint",
        ],
    );
    let computation = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/plan_computation.rs"
    );
    let defaults = computation
        .split("pub(crate) fn evaluate_default_calling_application(")
        .nth(1)
        .unwrap()
        .split("pub fn prepare_boundary_calling_plans<")
        .next()
        .unwrap();
    assert_absent(
        &[defaults],
        "default calling selection rescans source",
        &[".traits()", ".machines()", "trait_machine_signatures"],
    );
}

#[test]
fn terminal_calling_activation_uses_only_canonical_geometry() {
    let execution = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/terminal_execution.rs"
    );
    let protocol = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/policy_signature.rs"
    );
    let projection = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/terminal_signature.rs"
    );
    assert_absent(
        &[execution, protocol, projection],
        "calling activation retains source correspondence",
        &[
            "MaterializedBoundarySignature",
            "TypedTrees",
            "SymbolHandle",
            "TypeReference",
            "checked_trees",
            "layout_data_symbol",
        ],
    );
    assert_absent(
        &[projection],
        "terminal projection re-derives the policy signature:",
        &["policy_signature()?"],
    );
}

#[test]
fn calling_policy_selection_consumes_prepared_psi_applications() {
    let computation = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/plan_computation.rs"
    );
    assert_absent(
        &[computation],
        "target calling policy selection reopens source relationships",
        &[
            "standard_calling_vocabularies",
            "trait_requirements",
            "boundary_policy_instances",
            "collect_boundary_signatures",
            "AppliedTraitScope",
            "trait_implementation_applications",
            "captured_machine_applications",
            "evaluate_machine_symbol_for_invocation_measured(",
        ],
    );
    let callback = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/callback_bindings.rs"
    );
    assert_absent(
        &[callback],
        "callback bindings evaluate a materialized plan:",
        &["evaluate_materialized_calling_policy_plan("],
    );
    // Calling-policy code is produced through one Terminal production request;
    // the planner only names the plan it needs.
    let code = include_str!("terminal/calling_policy_code.rs");
    assert_eq!(code.matches("TerminalProductionRequest::new(").count(), 1);
    let execution =
        include_str!("../../realization/provider-planning/src/calling_policy_plans/policy_code.rs");
    assert_absent(
        &[execution],
        "planner requests Terminal production:",
        &["TerminalProductionRequest"],
    );
}

#[test]
fn provider_derivation_consumes_the_prepared_service_census() {
    let derivation = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/plan_derivation.rs"
    );
    let named = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/named_provider_plans.rs"
    );
    let service_catalog =
        include_str!("../../realization/provider-planning/src/service_declarations.rs");
    assert_absent(
        &[derivation, named, service_catalog],
        "provider census consumer reopens",
        &[
            "projection::from_typed",
            "service_schema::from_typed",
            "service_schema::from_sealed_accessor",
        ],
    );
    let native_supply =
        include_str!("../../realization/provider-planning/src/native_bindings/plans.rs");
    let settlement = include_str!("../../realization/provider-planning/src/provider_settlement.rs");
    let entry = include_str!("../../realization/provider-planning/src/program_entry.rs");
    let nomination =
        include_str!("../../realization/provider-planning/src/program_entry/discovery.rs");
    let adapter = include_str!(
        "../../realization/provider-planning/src/provider_planning/provenance_replay/adapter_conformance.rs"
    );
    assert_absent(
        &[native_supply, settlement, entry, nomination, adapter],
        "provider census consumer reopens",
        &["service_schema::from_typed"],
    );
    // The supplied target is evaluated before the selected target is compared.
    assert_in_order(
        native_supply,
        "native binding supply",
        &[
            "let evaluated = evaluate(",
            "if selected_target != Some(target)",
        ],
    );
    let build = include_str!("../../build-evaluation/src/admission/checking.rs");
    assert_in_order(
        build,
        "Build admission checking",
        &[
            "PreparedServiceCatalog::prepare(context)",
            "derive_satisfies_plans(",
        ],
    );
}

#[test]
fn service_catalog_consumers_do_not_keep_retired_binding_projection() {
    let dispatch = include_str!(
        "../../realization/selected-dispatch/src/checked_interpreter_dispatch/boundary_dispatch.rs"
    );
    assert_absent(
        &[dispatch],
        "dispatch retained",
        &[
            "classify_exact_bound_service_carrier(",
            "classify_exact_service_parameter_carrier(",
            "service_schema::from_typed",
            "provider_receiver_application(",
        ],
    );
}

#[test]
fn selected_calling_consumes_source_correspondence_without_ast_search() {
    let calling = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/selected_application.rs"
    );
    let selection = calling
        .split("pub(crate) fn selected_source_calling_application(")
        .nth(1)
        .unwrap()
        .split("/// Materialize this producer")
        .next()
        .unwrap();
    assert_absent(
        &[selection],
        "selected calling reopens",
        &[
            ".traits()",
            ".operators()",
            ".machines()",
            "trait_machine_signatures(",
            "schema_binds_exact_",
            "service_schema::from_typed",
            "matching_methods",
        ],
    );
}

#[test]
fn native_product_supplies_inputs_without_orchestrating_component_emission() {
    let source = include_str!("product_execution.rs");
    let inputs = include_str!("../../../application/build/src/native_completion.rs");
    assert_absent(
        &[source],
        "compiler still drives",
        &[
            "native_production::prepare(&terminal, request)",
            "build_evaluation::scalar_completion::complete(",
            "native_production::compile(prepared)",
        ],
    );
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
fn selected_numeric_admission_binds_facts_after_requiring_the_owner() {
    let preparation = include_str!("../../realization/selected-dispatch/src/provider_binding.rs");
    assert_in_order(
        preparation,
        "selected provider binding",
        &[".require_owner(", "bind_selected_provider_plan_facts("],
    );
}

#[test]
fn boundary_application_policy_has_one_source_free_owner() {
    let settlement = include_str!(
        "../../pipeline/00_terminal-psi-to-abstract-operations/src/boundary_application_settlement.rs"
    );
    assert_absent(
        &[settlement],
        "boundary application settlement owns",
        &["ProviderBinding::"],
    );
}

#[test]
fn terminal_consumers_call_psi_without_a_product_wrapper() {
    let inspection = include_str!("product_execution/inspection.rs");
    assert_absent(
        &[inspection],
        "inspection requests Terminal production:",
        &["TerminalProductionRequest", ".produce("],
    );
    // One product production attempt per first lowering leg.
    let observations = include_str!("observations.rs");
    assert_eq!(
        observations.matches("record_terminal_production()").count(),
        1
    );
}

#[test]
fn exclusion_source_resolution_stays_out_of_terminal_policy() {
    let policy = include_str!("terminal/behavior_exclusions.rs");
    assert_absent(
        &[policy],
        "crossed the Terminal policy boundary:",
        &[
            "typed_trees::",
            "checked_trees::",
            "resolve_product_path(",
            "trust_model::",
        ],
    );
}

#[test]
fn terminal_composition_admission_follows_canonical_publication() {
    let producer = include_str!(
        "../../../../psi/compilation/driver/terminal-production/src/terminal_production.rs"
    );
    assert_eq!(producer.matches("application.attach(").count(), 1);
    assert_in_order(
        producer,
        "application attachment",
        &["finalize_terminal_program(", "application.attach("],
    );
    assert_absent(
        &[producer],
        "Psi production runs Omega policy:",
        &[
            "admission.admit(",
            "policy.admit(",
            "retained_source_machine(",
        ],
    );
    let composition = include_str!(
        "../../../../psi/compilation/driver/terminal-production/src/composition_roots.rs"
    );
    assert_absent(
        &[composition],
        "Psi composition runs Omega policy:",
        &[
            "admission.admit(",
            "for machine in std::iter::once(selection.machine)",
        ],
    );
    let attachment = composition.split("pub(crate) fn attach(").nth(1).unwrap();
    assert_absent(
        &[attachment],
        "post-publication application attachment reopens source",
        &[
            "CheckedTrees",
            "SymbolHandle",
            "select_terminal_machine",
            "exact_owner_nominal_identity",
        ],
    );
    assert_in_order(
        attachment,
        "application roots",
        &[
            "roots.push(project_root(module, provenance, entry))",
            "callbacks.retained_roots(module)",
            "if let Some(adapter)",
        ],
    );
    let product = include_str!("terminal/terminal_artifact.rs");
    assert!(!product.contains("machines.push(first.selected_machine)"));
    assert_in_order(
        product,
        "terminal product",
        &[
            ".into_shared_portable()",
            "program.attach_application(",
            "policy.admit(",
        ],
    );
    let custody =
        include_str!("../../../../psi/compilation/driver/terminal-production/src/custody.rs");
    let portable = custody
        .split("impl PortableTerminalProgram {")
        .nth(1)
        .unwrap();
    assert_absent(
        &[portable],
        "late application attachment reopens source:",
        &["CheckedTrees", "SymbolHandle", "TerminalProductionRequest"],
    );
    assert!(portable.contains("&self.provenance"));
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
    assert_in_order(
        policy,
        "behavior exclusion admission",
        &[
            "let first = first?;",
            "close_checked_provider_specializations(",
            "for root in std::iter::once(Ok(first)).chain(roots)",
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
fn callback_root_policy_consumes_prepared_source_correspondence() {
    let preparation = include_str!("../../realization/provider-planning/src/provider_instances.rs");
    assert_absent(
        &[preparation],
        "callback roots reopen source:",
        &[
            "program.callback_declaration(usage)",
            "program.facts.nominal_machine_uses",
        ],
    );
    assert_eq!(
        preparation
            .matches("exact_checked_adapter(context, plan, row)")
            .count(),
        1
    );
}

#[test]
fn provider_instance_policy_consumes_checked_declaration_groups() {
    let preparation = include_str!("../../realization/provider-planning/src/provider_instances.rs");
    assert_absent(
        &[preparation],
        "provider instances reopen source:",
        &["machine_specializations", "provider_closed_requirements"],
    );
}

#[test]
fn provider_demand_is_prepared_by_psi_not_each_target_settlement() {
    let settlement = include_str!("../../realization/provider-planning/src/provider_settlement.rs");
    assert_absent(
        &[settlement],
        "target settlement recomputes provider demand:",
        &[
            "collect_program_references(",
            "provider_closed_requirements",
        ],
    );
    let publication = include_str!(
        "../../realization/provider-planning/src/target_machines/provider_publication.rs"
    );
    assert_absent(
        &[publication],
        "target publication recomputes provider demand:",
        &["is_candidate_demanded"],
    );
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
    assert_absent(
        &[selection],
        "target defaults rescan source:",
        &[".machines()", "ProviderPlanningContext"],
    );
}

#[test]
fn target_body_policy_consumes_psi_direct_call_declarations() {
    let bridge = include_str!("../../realization/provider-planning/src/target_machines.rs");
    assert_absent(
        &[bridge],
        "target-body bridge rescans source:",
        &[
            "ExplicitStateCallees",
            ".machine_states(",
            "fn called_machine(",
        ],
    );
}

#[test]
fn boundary_value_layout_consumes_source_free_recipes() {
    let materialization = include_str!("../../backend/layout/src/boundary_values.rs");
    assert_absent(
        &[materialization],
        "boundary value layout reopens source:",
        &[
            "BoundaryOpaqueRepresentationUse",
            "OpaqueRepresentationSelection",
            "representation_selections",
        ],
    );
}
