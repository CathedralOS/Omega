//! Source guards for the application/compiler ownership boundary.

#[test]
fn package_audit_owns_session_and_target_execution_in_build() {
    let mut application = include_str!("package_inspection.rs");
    for operation in [
        "PackageInspectionTransaction::open(options)",
        "BuildSession::create(",
        "transaction.prepare(None)",
        "resolve_inspection_sources(&inspection, &mut session)",
        "for target in inspection.targets().to_vec()",
        "inspection.review_target(",
        "inspection.finish()",
        "session.dispose(result)",
    ] {
        let position = application
            .find(operation)
            .expect("ordered audit lifecycle");
        application = &application[position + operation.len()..];
    }
    let policy =
        include_str!("../../../packages/package-manager/src/operations/inspect_packages.rs");
    for forbidden in [
        "PackageBuildSession::create(",
        "session.dispose(",
        "for target in targets",
    ] {
        assert!(
            !policy.contains(forbidden),
            "package policy schedules {forbidden}"
        );
    }
    assert!(policy.contains("self.completed_targets != self.targets.len()"));
    assert!(policy.contains("after != self.before"));
}

#[test]
fn review_policy_admits_work_and_application_executes_it() {
    let policy = include_str!(
        "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
    );
    for forbidden in [
        ".prepare_package_build(",
        ".begin_package_build(",
        ".complete_package_build(",
        ".review_package_build(",
        ".publish_component(",
        "PendingReviewBuild",
        "run_closure_pass(",
        "into_execution_request",
        "reserve_build_directory",
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
        ".require_execution_contract(",
        ".prepare_package_product(",
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
    assert!(!execution.contains("execute_package_build("));
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
        "resolve_command_sources(preparation, session)",
        "for request in prepared.review_requests()",
        "request.review(",
        "prepared.complete(reviews, &mut session.packages)",
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
    assert!(cli.contains("use omega_build::{BuildSession, execute_package_command};"));
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
fn source_and_package_execution_have_one_application_owner() {
    let source = include_str!("source_compilation.rs");
    for operation in [
        "compiler.begin_compilation(",
        "compiler.complete_compilation(",
        "compiler.begin_source_compilation(",
        "compiler.complete_source_check(",
        "compiler.complete_source_product(",
        "crate::build_execution::execute_build(",
    ] {
        // rustfmt may put a receiver and its method on different lines.
        assert!(
            source
                .split_whitespace()
                .collect::<String>()
                .contains(&operation.split_whitespace().collect::<String>()),
            "source orchestration lost {operation}"
        );
    }
    for compiler in [
        include_str!("../../../compilation/driver/src/compiler.rs"),
        include_str!("../../../compilation/driver/src/checked/checking.rs"),
        include_str!("../../../compilation/driver/src/checked/compilation_session.rs"),
    ] {
        for execution in [
            ".host()",
            "host.start(",
            "activation.advance(",
            "compile_with_session(",
        ] {
            assert!(
                !compiler.contains(execution),
                "compiler executes Build: {execution}"
            );
        }
    }
    let package = include_str!("build_execution.rs");
    assert!(package.contains("execute_build(invocation.execution())"));
}

#[test]
fn build_entrance_shows_the_ordered_project_lifecycle() {
    let mut source = include_str!("build.rs");
    for operation in [
        "build_directory::admit_outputs(",
        "begin_local_project_preparation(",
        "crate::resolve_project_sources(",
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
        .find("package_manager::operations::admit_project_products(")
        .unwrap();
    let compile = execution
        .find("crate::package_compilation::compile_returned_packages(")
        .unwrap();
    let reconcile = execution
        .find("admitted.completion.complete(products, session.packages)")
        .unwrap();
    assert!(admit < compile && compile < reconcile);
    let preparation =
        include_str!("../../../packages/package-manager/src/operations/prepare_project.rs");
    assert!(!preparation.contains("resolve_static_source_closure"));
    let build_exports = include_str!("lib.rs");
    for removed in [
        "mod check_project",
        "mod native_project",
        "PreparedLocalProjectNativeRequest",
        "PreparedLocalProjectCheckRequest",
    ] {
        assert!(
            !build_exports.contains(removed),
            "obsolete Build facade: {removed}"
        );
    }
    for example in [
        include_str!("../examples/package_batch.rs"),
        include_str!("../../cli/examples/qualify_inputs.rs"),
    ] {
        assert!(example.contains("build(BuildRequest {"));
        assert!(!example.contains("prepare_local_project"));
    }
}

#[test]
fn application_owns_the_compiler_separately_from_package_resources() {
    let resources = include_str!("../../../packages/package-manager/src/build_session.rs");
    assert!(!resources.contains("CompilationSession"));
    assert!(!resources.contains("fn compiler("));
    let owner = include_str!("build_session.rs");
    assert!(owner.contains("compiler: CompilationSession"));
    assert!(owner.contains("packages: PackageBuildSession"));
    for entrance in [
        include_str!("package_commands.rs"),
        include_str!("package_inspection.rs"),
        include_str!("project_compilation.rs"),
        include_str!("build_execution/source_resolution.rs"),
    ] {
        assert!(entrance.contains("BuildSession"));
        assert!(!entrance.contains("CompilationSession::default()"));
    }
}

#[test]
fn application_schedules_dependencies_without_opening_source_continuations() {
    let execution = include_str!("build_execution.rs");
    assert!(execution.contains("while !frontiers.is_empty()"));
    assert!(execution.contains("join_generated_sources(occurrence, &frontier.products)"));
    assert!(execution.contains("occurrence.prepare(compiler, completed)"));
    assert!(
        execution.contains("Self::Execute(request) => execute_package_build(compiler, request)")
    );
    assert!(execution.contains("source.require_execution_contract(&request.execution)?"));
    assert!(execution.contains("compiler.begin_package_build(request)"));
    assert!(execution.contains("activation.advance(1024)"));
    assert!(execution.contains("compiler.complete_package_build(invocation, returned)"));
    assert!(execution.contains("compiler.complete_package_components(builds)"));
    for source in [
        execution,
        include_str!("package_compilation.rs"),
        include_str!("build_execution/source_resolution.rs"),
    ] {
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
