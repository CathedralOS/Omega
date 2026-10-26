//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

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
    assert!(application.contains("crate::package_compilation::compile_returned_packages("));
    assert!(application.contains("admitted.completion.complete(products, session.packages)"));
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
    assert!(completion.contains("publish_checked_component_products(checked)"));
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
        include_str!("compiler/package.rs"),
    ] {
        assert!(!source.contains("map(|_| None)"));
        assert!(!source.contains("outcomes["));
        assert!(!source.contains(".zip("));
    }
}

#[test]
fn native_product_consumes_terminal_without_producing_it() {
    let source = include_str!("native.rs");
    assert!(source.contains("terminal: compilation_products::RetainedTerminalArtifact"));
    assert!(!source.contains("crate::terminal::"));
    assert!(!source.contains("TerminalProductionRequest"));
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
    assert!(constant.contains(".invoke(&mut meter)"));
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
