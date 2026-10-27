//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

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
    let closure = include_str!(
        "../../../../psi/compilation/compile-time/src/const_evaluation/retained_constants.rs"
    );
    for source in [evaluation, queries, closure] {
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
fn deferred_constant_custody_and_value_publication_belong_to_psi() {
    let checking = include_str!("checked/checking/source_checking.rs");
    let closure = include_str!("execution_settlement/constant_recipe_closure.rs");
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
    assert!(closure.contains("evaluation.close_constants("));
    assert!(product.contains("RetainedConstantEvaluation"));
    let checking = include_str!("../../../../psi/compilation/driver/src/source_checking.rs");
    assert!(
        checking
            .find("RetainedConstantEvaluation::capture(")
            .unwrap()
            < checking.find("prepare_source_completion(").unwrap()
    );
}

#[test]
fn reused_product_selection_keeps_current_deferred_work() {
    use std::sync::Arc;

    let (syntax, pending) = build_time_evaluation::evaluate_pre_resolution(
        build_time_evaluation::BuildTimeEvaluationRequest {
            syntax_trees: Default::default(),
            source_context: Default::default(),
        },
    )
    .expect("empty elaboration")
    .into_syntax_and_pre_check();
    drop(syntax);
    let program = Arc::new(checked_trees::CheckedTrees::default());
    let constants =
        build_time_evaluation::RetainedConstantEvaluation::capture(&program.typed, vec![pending])
            .expect("retained continuation");
    let mut selections = psi_compiler::ProductSelections::default();
    let first = selections
        .select(
            psi_compiler::CheckedSource {
                program: Arc::clone(&program),
                constants: None,
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
                constants: Some(constants.clone()),
                retained: None,
                reused: true,
            },
            None,
        )
        .expect("reused root selection");
    assert!(Arc::ptr_eq(&first.program, &second.program));
    assert!(std::ptr::eq(
        constants.source(),
        second.constants.as_ref().expect("current work").source()
    ));
    let third = selections
        .select(
            psi_compiler::CheckedSource {
                program,
                constants: None,
                retained: None,
                reused: true,
            },
            None,
        )
        .expect("no current deferred work");
    assert!(Arc::ptr_eq(&first.program, &third.program));
    assert!(third.constants.is_none());
}

#[test]
fn constant_proof_closure_belongs_to_psi() {
    let coordinator = include_str!("execution_settlement/constant_recipe_closure.rs");
    assert!(coordinator.contains("psi_compiler::ConstantProofClosure::new("));
    for forbidden in [
        "discharge_all(",
        "CheckedTrees::with_roots(",
        "settle_closed_constant_execution(",
        "closed_facts",
        "decided_formations",
    ] {
        assert!(
            !coordinator.contains(forbidden),
            "coordinator owns {forbidden}"
        );
    }
    let facade = include_str!("../../../../psi/compilation/driver/src/lib.rs");
    assert!(!facade.contains("settle_closed_constant_execution"));
    let proof =
        include_str!("../../../../psi/compilation/driver/src/source_checking/constant_closure.rs");
    for forbidden in [
        "provider_planning::",
        "selected_dispatch::",
        "target::",
        "native_production::",
    ] {
        assert!(
            !proof.contains(forbidden),
            "Psi proof closure depends on {forbidden}"
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
        "with_complete_dependency_generated_sources(",
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
fn terminal_cohorts_retain_admitted_applications() {
    let source = include_str!("terminal/terminal_artifact.rs");
    assert!(source.contains("struct AdmittedTerminalApplication"));
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
fn terminal_production_owns_the_build_provider_companion() {
    let production = include_str!("terminal/terminal_artifact.rs");
    let completion = include_str!("../../native-production/src/application.rs");
    assert!(completion.contains("pub provider_proposals:"));
    assert!(production.contains("provider_proposals: produce_target_provider_proposals("));
    assert!(production.contains("TargetProviderPublicationRequest"));
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
    assert!(source.contains("native_production::compile(terminal, request)"));
    let inputs = include_str!("product_execution/native_inputs.rs");
    assert!(inputs.contains("ComponentCompilation"));
    assert!(inputs.contains("receiving_identities: facts.identities()"));
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
fn selected_execution_does_not_copy_checked_programs() {
    for source in [
        include_str!("../../realization/selected-dispatch/src/selected_dispatch.rs"),
        include_str!("../../realization/selected-dispatch/src/boundary_dispatch.rs"),
    ] {
        assert!(!source.contains("Arc::make_mut"));
    }
}

#[test]
fn terminal_consumers_call_psi_without_a_product_wrapper() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("terminal-production ="));
    let inspection = include_str!("inspection.rs");
    let product = include_str!("terminal/terminal_artifact.rs");
    for source in [inspection, product] {
        assert!(source.contains("TerminalProductionRequest"));
        assert!(source.contains(".produce("));
        assert!(!source.contains("produce_terminal_artifact("));
    }
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
    assert!(selection.contains("exclusions::resolve("));
    let source = include_str!("checked/checking/product_selections/exclusions.rs");
    assert!(source.contains("pub(super) fn resolve("));
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
