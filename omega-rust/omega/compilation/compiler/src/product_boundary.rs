// We enforce source-level ownership and operation-ordering boundaries across
// crates that legitimately depend on lower stage owners (`psi_compiler`,
// `provider_planning`, `selected_dispatch`, `compilation_products`). Crate-level
// dependency direction is checked separately by `mbx xtask check-boundary`;
// these tests verify that coordinator modules do not call internal stage
// operations out of order or reopen source representations after publication.

fn assert_absent(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(!source.contains(token), "{complaint} {token}");
        }
    }
}

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
    let checking = include_str!("../../../../psi/compilation/psi-compiler/src/source_checking.rs");
    assert_in_order(
        checking,
        "Psi source checking",
        &[
            "self.admission.representations.admit(&program)",
            "validation::source_completion::prepare_source_completion(",
            "prior.judgment.judges_preparation_input(",
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
    let source = include_str!("../../../../psi/compilation/psi-compiler/src/source_compilation.rs");
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
        &["resolve_product_path(", "WireCompatibilityFact"],
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
    let selection =
        include_str!("../../../build-system/build-settings/src/admission/selected_constants.rs");
    assert_absent(&[selection], "selection drives", &[".finish(typed)"]);
    let geometry = include_str!("../../realization/provider-planning/src/data_layout_queries.rs");
    assert_absent(
        &[geometry],
        "geometry owns",
        &["expression_mut(", "proof_only::", "PreCheckEvaluation"],
    );
}

#[test]
fn canonical_proof_completion_has_no_source_rewrite_api() {
    let coordinator =
        include_str!("../../../build-system/build-settings/src/activation/scalar_completion.rs");
    assert_absent(
        &[coordinator],
        "coordinator owns",
        &[
            "CheckedTrees::with_roots(",
            "decided_formations",
            "machine_authored_target(",
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
            include_str!("../../../packages/package-manager/src/review/candidate/passes.rs"),
            include_str!("../../../packages/package-manager/src/review/candidate/review_cache.rs"),
        ],
        "package review interprets",
        &[
            "compiler::CheckedCompilation",
            ".check_package_build(",
            ".execution()",
            ".build_admission()",
            "project_checked_package_policy(",
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
            include_str!("../../../build-system/orchestration/src/build.rs"),
            include_str!("../../../build-system/orchestration/src/project/project_compilation.rs"),
            include_str!("../../../build-system/orchestration/src/project/products.rs"),
        ],
        "Build operation drives",
        &[
            ".admit_trust(",
            "ProductReportPlan",
            "CheckedProductReport::",
            "CheckedCompilation",
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
            "with_complete_dependency_build_handoffs(",
            "with_independent_component_descriptions(",
            "CheckedCompilation",
            ".compile_packages(",
            "CompilationSession",
        ],
    );
}

#[test]
fn package_frontiers_batch_components_without_nested_source_workers() {
    assert_absent(
        &[
            include_str!("../../../build-system/orchestration/src/compile/build_execution.rs"),
            include_str!("checked/product_batch.rs"),
        ],
        "package batch nests source work:",
        &["run_on_compile_thread"],
    );
}

#[test]
fn native_provider_handoff_borrows_closed_source_associations() {
    let projection = include_str!("../../products/src/checked_compilation.rs");
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
}

#[test]
fn component_handoff_retains_the_producing_join() {
    let components =
        include_str!("../../../build-system/orchestration/src/compile/native_completion.rs");
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
fn terminal_calling_activation_uses_only_canonical_geometry() {
    let execution =
        include_str!("../../../build-system/build-settings/src/calling_policy_execution.rs");
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
fn native_telescope_construction_is_shared_by_canonical_replay_and_projection() {
    let replay = include_str!(
        "../../backend/calling-conventions/src/boundary_application/terminal_telescope.rs"
    );
    let projection = include_str!(
        "../../realization/provider-planning/src/calling_policy_plans/terminal_signature.rs"
    );
    for consumer in [replay, projection] {
        assert_eq!(
            consumer.matches("NativeParameterTelescope::close(").count(),
            1
        );
        assert_absent(
            &[consumer],
            "native telescope construction duplicated in a consumer:",
            &["sort_unstable_by_key(|parameter| parameter.native_ordinal)"],
        );
    }
}

#[test]
fn native_product_supplies_inputs_without_orchestrating_component_emission() {
    let source = include_str!("product_execution.rs");
    let inputs =
        include_str!("../../../build-system/orchestration/src/compile/native_completion.rs");
    assert_absent(
        &[source],
        "compiler still drives",
        &[
            "native_compiler::prepare(&terminal, request)",
            "build_settings::scalar_completion::complete(",
            "native_compiler::compile(prepared)",
        ],
    );
    assert_absent(
        &[source, inputs],
        "product owner orchestrates",
        &["with_component_products("],
    );
}

#[test]
fn boundary_application_policy_has_one_source_free_owner() {
    let settlement =
        include_str!("../../representations/abstract-operations/verified-input/src/settlement.rs");
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
    let producer =
        include_str!("../../../../psi/compilation/psi-compiler/terminal-production/src/lib.rs");
    assert_eq!(producer.matches("application.attach(").count(), 1);
    assert_in_order(
        producer,
        "application attachment",
        &["finalize_terminal_program(", "application.attach("],
    );
    assert_absent(
        &[producer],
        "Psi production runs Omega policy:",
        &["admission.admit(", "policy.admit("],
    );
    let composition =
        include_str!("../../../../psi/compilation/psi-compiler/terminal-production/src/lib.rs");
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
            "let produced = produce_shared_terminal(",
            ".attach_application(",
            "policy.admit(",
        ],
    );
    let custody =
        include_str!("../../../../psi/compilation/psi-compiler/terminal-production/src/custody.rs");
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
fn target_body_policy_consumes_psi_direct_call_declarations() {
    let bridge = include_str!("../../realization/provider-planning/src/target_machines.rs");
    assert_absent(
        &[bridge],
        "target-body bridge rescans source:",
        &["ExplicitStateCallees", ".machine_states("],
    );
}

#[test]
fn selected_callback_custody_carries_declarations_not_source_symbols() {
    let selection = include_str!("../../realization/selected-dispatch/src/selected_execution.rs");
    assert!(selection.contains("pub callback_roots: terminal_psi::PreparedTerminalRoots"));
    let custody =
        include_str!("../../../../psi/compilation/psi-compiler/terminal-production/src/custody.rs");
    assert!(!custody.contains("pub admission_roots: &'a [symbols::SymbolHandle]"));
    let preparation =
        include_str!("../../../../psi/compilation/representations/terminal-psi/src/lib.rs");
    assert!(!preparation.contains("symbols::SymbolHandle"));
    assert!(preparation.contains("declaration.same_identity_as("));
    assert!(preparation.contains("pub fn new("));
    let compact_preparation = preparation
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    for owner in [
        "declaration.machine.identity()",
        "declaration.state.identity()",
        "declaration.semantic_declaration",
        "specialization.template.identity()",
        "specialization.template_semantic_declaration",
    ] {
        assert!(compact_preparation.contains(&format!("identities.is_invocation_local({owner})")));
    }
    assert!(preparation.contains("selected root has multiple exact retained declarations"));
    assert_absent(
        &[preparation],
        "Terminal root preparation reopens source",
        &["checked_trees", "typed_trees", "psi_terminal_production"],
    );
    let admission =
        include_str!("../../../../psi/compilation/psi-compiler/terminal-production/src/lib.rs");
    assert!(admission.contains("roots.push(project_root(module, provenance, entry))"));
    assert!(admission.contains("callbacks.retained_roots(module)"));
}

#[test]
fn service_surface_projection_is_psi_owned_and_calling_is_an_omega_attachment() {
    let surface =
        include_str!("../../../../psi/foundation/language-semantics/src/service_surface.rs");
    assert!(!surface.contains("BoundaryCallingPlanCommitment"));
    assert!(!surface.contains("calling_plan_report_fingerprint"));
    let projection = include_str!(
        "../../../../psi/compilation/representations/typed-trees/src/service_declarations.rs"
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
    let catalog = include_str!("../../realization/provider-planning/src/service_declarations.rs");
    assert!(catalog.contains("projected.exact_requirements()?"));
    assert!(!catalog.contains("trait_requirements("));
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
