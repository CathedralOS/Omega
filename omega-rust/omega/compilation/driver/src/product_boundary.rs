//! Guard the production boundaries already owned by this driver. Shared
//! representations remain legitimate dependencies; transform crates do not.

#[test]
fn application_does_not_depend_on_pipeline_stages() {
    let manifest = include_str!("../../../application/cli/Cargo.toml");
    assert!(
        !manifest.contains("/pipeline/"),
        "application operations must call compiler drivers, not numbered transforms"
    );
}

#[test]
fn package_operations_do_not_assemble_compiler_products() {
    for source in [
        include_str!("../../../packages/package-manager/src/operations/check_project.rs"),
        include_str!("../../../packages/package-manager/src/operations/compile_project.rs"),
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
                "package operation drives {forbidden}"
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
fn package_target_batches_submit_source_work_together() {
    for source in [
        include_str!("../../../packages/package-manager/src/operations/check_project.rs"),
        include_str!("../../../packages/package-manager/src/operations/compile_project.rs"),
    ] {
        assert!(source.contains("compile_accepted_projects(requests, &mut session)"));
    }
    let source =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    assert!(source.contains("session.compiler.compile_packages(plans)"));
    assert!(!source.contains(".check_package("));
}

#[test]
fn package_frontiers_batch_components_without_nested_source_workers() {
    let preparation = include_str!("checked/package_preparation.rs");
    assert!(preparation.contains(".select_build_batch(builds)"));
    assert!(preparation.contains("publish_checked_components(checked)"));
    assert!(!preparation.contains("self.check_build("));
    assert!(!preparation.contains("run_on_compile_thread"));
    let batch = include_str!("checked/product_batch.rs");
    assert!(!batch.contains("run_on_compile_thread"));
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
    assert!(compiler.contains("ProductRequest::plan"));
    assert!(compiler.contains("produce_retained_terminal_artifacts(terminal_requests)"));
    assert!(!compiler.contains("map(|_| None)"));
    assert!(!compiler.contains("outcomes["));
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
fn retained_runtime_borrows_terminal_storage() {
    let source = include_str!("runtime.rs");
    assert!(source.contains("TerminalProgram::borrow(artifact)"));
    assert!(!source.contains("TerminalProgram::load("));
    assert!(!source.contains("semantic_bytes()"));
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
            "../../../packages/package-manager/src/review/candidate/compilation/package_pass.rs"
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
