//! Source guards for the application/compiler ownership boundary.

#[test]
fn review_policy_admits_work_and_application_executes_it() {
    let policy = include_str!(
        "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
    );
    for forbidden in [
        ".prepare_package_build(",
        ".review_package_build(",
        ".publish_component(",
        "PendingReviewBuild",
        "run_closure_pass(",
    ] {
        assert!(
            !policy.contains(forbidden),
            "package policy executes {forbidden}"
        );
    }
    let execution = include_str!("package_review.rs");
    for operation in [
        "plan.begin_round(",
        "plan.occurrences()",
        "round.admit_occurrence(",
        ".prepare_package_build(",
        ".review_package_build(",
        ".publish_component(",
        "round.retain(",
    ] {
        assert!(
            execution.contains(operation),
            "application is missing {operation}"
        );
    }
    assert!(execution.contains("pending_builds.push(PendingReviewBuild"));
    assert!(execution.contains("recovered.push(round.into_retained())"));
    assert!(execution.contains("completion.reserve_build_directory(session)"));
    let retention = execution.find("round.retain(").unwrap();
    let cache_publication = execution.find("cache_write.component_source()").unwrap();
    assert!(
        retention < cache_publication,
        "policy admits optional cache work first"
    );
    let audit = include_str!("../../cli/src/cli/packages/audit.rs");
    assert!(audit.contains("use omega_build::inspect_packages;"));
}

#[test]
fn package_commands_schedule_work_outside_package_policy() {
    let mut execution = include_str!("package_commands.rs");
    for operation in [
        "PackageCommandTransaction::open(options)",
        "transaction.prepare(command, None)",
        "for request in prepared.review_requests()",
        "request.review(",
        "prepared.complete(reviews, session)",
    ] {
        let position = execution
            .find(operation)
            .expect("ordered package command lifecycle");
        execution = &execution[position + operation.len()..];
    }
    let policy = include_str!("../../../packages/package-manager/src/package_manager.rs");
    for forbidden in ["execute_package_command(", "CompilationSession"] {
        assert!(
            !policy.contains(forbidden),
            "package policy schedules {forbidden}"
        );
    }
    let cli = include_str!("../../cli/src/cli/packages/mod.rs");
    assert!(cli.contains("use omega_build::execute_package_command;"));
}

#[test]
fn application_depends_on_facades_not_language_stages() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "syntax-trees",
        "typed-trees",
        "checked-trees",
        "lowered-psi",
        "pipeline/",
        "provider-planning",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "Build reaches behind a facade: {forbidden}"
        );
    }
    let cli = include_str!("../../cli/src/cli/compilation.rs");
    assert!(cli.contains("use omega_build::{"));
    assert!(!cli.contains("prepare_local_project_for_targets("));
    assert!(!cli.contains("publish_compilation("));
    let execution = include_str!("../../cli/src/execution/compilation.rs");
    assert!(execution.contains("omega_build::build("));
    assert!(!execution.contains("prepare_local_project("));
    assert!(!execution.contains("compiler::compile("));
    let inspection = include_str!("../../cli/src/inspection/mod.rs");
    assert!(inspection.contains("omega_build::build("));
    assert!(!inspection.contains("prepare_local_project("));
    assert!(!inspection.contains("CompilationSession::default()"));
}

#[test]
fn build_entrance_shows_the_ordered_project_lifecycle() {
    let mut source = include_str!("build.rs");
    for operation in [
        "build_directory::admit_outputs(",
        "prepare_local_project_for_targets(",
        "products::compile(",
        "publication::publish_products(",
    ] {
        let position = source
            .find(operation)
            .expect("operation follows its inputs");
        source = &source[position + operation.len()..];
    }
    let products = include_str!("products.rs");
    assert!(
        !products.contains(".zip("),
        "product identity comes from its outcome"
    );
}

#[test]
fn ordinary_package_operations_admit_instead_of_execute() {
    let admission =
        include_str!("../../../packages/package-manager/src/operations/accepted_compilation.rs");
    for forbidden in [
        ".compile_packages(",
        ".check_package(",
        "CompilationSession",
    ] {
        assert!(
            !admission.contains(forbidden),
            "admission executes {forbidden}"
        );
    }
    let exports = include_str!("../../../packages/package-manager/src/operations/mod.rs");
    for removed in [
        "mod compile_project",
        "mod check_project",
        "compile_prepared_local_project",
    ] {
        assert!(
            !exports.contains(removed),
            "obsolete package execution route: {removed}"
        );
    }
    let execution = include_str!("project_compilation.rs");
    let admit = execution
        .find("admit_project_compilation(request)")
        .unwrap();
    let compile = execution
        .find("crate::compile_packages(&mut compiler, admitted.requests)")
        .unwrap();
    let reconcile = execution
        .find("admitted.completion.complete(products)")
        .unwrap();
    assert!(admit < compile && compile < reconcile);
}

#[test]
fn application_schedules_dependencies_without_opening_source_continuations() {
    let execution = include_str!("build_execution.rs");
    assert!(execution.contains("while !frontiers.is_empty()"));
    assert!(execution.contains("join_generated_sources(occurrence, &frontier.products)"));
    assert!(execution.contains("compiler.prepare_package_build(occurrence)"));
    assert!(execution.contains("compiler.complete_package_components(builds)"));
    for source in [execution, include_str!("package_compilation.rs")] {
        for forbidden in [
            "CheckedCompilation",
            "PreparedPackageBuild",
            ".into_continuation(",
            ".select_build_batch(",
            "produce_products(",
        ] {
            assert!(
                !source.contains(forbidden),
                "application opens source: {forbidden}"
            );
        }
    }
    let compiler = include_str!("../../../compilation/driver/src/checked/package_compilation.rs");
    for removed in [
        "fn compile_packages(",
        "fn check_package(",
        "fn join_dependency_builds(",
        "PackageFrontier",
        "join_generated_sources(",
    ] {
        assert!(
            !compiler.contains(removed),
            "compiler still schedules {removed}"
        );
    }
}
