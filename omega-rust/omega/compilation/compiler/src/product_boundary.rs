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
fn opaque_representation_admission_is_not_rebuilt_in_the_coordinator() {
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
fn provider_instance_inspection_does_not_reopen_checked_facts() {
    let preparation = include_str!("../../products/src/provider_instances.rs");
    assert_absent(
        &[preparation],
        "provider instances reopen source:",
        &["machine_specializations", "provider_closed_requirements"],
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
