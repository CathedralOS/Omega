//! Source guards for the application/compiler ownership boundary.
//!
//! Which crates Build may depend on is checked by `tools/check_native_boundary.py`
//! from `cargo metadata`. What remains here is who schedules and executes work
//! inside crates that legitimately depend on each other, and the order of the
//! lifecycle steps. Behavior that holds by construction is not repeated: Build
//! executing once per compilation is counted by `tests/executed_commands.rs`.

/// Assert every `source` omits each listed token, naming the owner that must
/// not spell it.
fn assert_absent(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(!source.contains(token), "{complaint} {token}");
        }
    }
}

/// Assert the operations first appear in `source` in the listed order, ignoring
/// whitespace so rustfmt may break a receiver from its method.
fn assert_in_order(source: &str, owner: &str, operations: &[&str]) {
    let compact = source.split_whitespace().collect::<String>();
    let mut previous = None;
    for operation in operations {
        let position = compact
            .find(&operation.split_whitespace().collect::<String>())
            .unwrap_or_else(|| panic!("{owner} lost {operation}"));
        assert!(
            previous.is_none_or(|before| before < position),
            "{owner} runs {operation} out of order"
        );
        previous = Some(position);
    }
}

#[test]
fn readiness_retains_live_jobs_without_recursive_source_execution() {
    let source = include_str!("build_execution/source_resolution/readiness.rs");
    let scheduler = source
        .split_once("fn drive_work(")
        .unwrap()
        .1
        .split_once("fn build_request(")
        .unwrap()
        .0;
    assert_absent(
        &[scheduler],
        "recursive dependency scheduling:",
        &[
            "self.execute_source(",
            "self.resolve_graph(",
            "self.run_work(",
            "self.drive_work(",
        ],
    );
}

#[test]
fn readiness_prepares_helper_products_without_recursive_dependency_calls() {
    let source = include_str!("build_execution/source_resolution/readiness.rs");
    let preparation = source
        .split_once("fn prepare_product(")
        .expect("helper Product entrance")
        .1
        .split_once("fn prepared_product(")
        .expect("retained Product lookup")
        .0;
    assert_absent(
        &[preparation],
        "recursive dependency preparation:",
        &["self.prepare_product("],
    );
}

#[test]
fn package_audit_owns_session_and_target_execution_in_build() {
    assert_in_order(
        include_str!("package_inspection.rs"),
        "package audit",
        &[
            "PackageInspectionTransaction::open(options)",
            "BuildSession::create_retained(",
            "transaction.prepare(None)",
            "resolve_inspection_sources(&inspection, &mut session)",
            "for target in inspection.targets().to_vec()",
            "inspection.review_target(",
            "inspection.finish()",
            "session.dispose(result)",
        ],
    );
    let policy =
        include_str!("../../../packages/package-manager/src/operations/inspect_packages.rs");
    assert_absent(
        &[policy],
        "package policy schedules",
        &[
            "PackageBuildSession::create(",
            "session.dispose(",
            "for target in targets",
        ],
    );
}

#[test]
fn review_policy_admits_work_and_application_executes_it() {
    let policy = include_str!(
        "../../../packages/package-manager/src/review/candidate/passes/occurrences.rs"
    );
    assert_absent(
        &[policy],
        "package policy executes",
        &[
            ".prepare_package_build(",
            ".begin_package_build(",
            ".complete_package_build(",
            ".review_package_build(",
            ".publish_component(",
            "PendingReviewBuild",
            "run_closure_pass(",
            "into_execution_request",
            "reserve_build_directory",
        ],
    );
    let execution = include_str!("package_review.rs");
    assert_absent(
        &[execution],
        "application bypasses the review round:",
        &["execute_package_build("],
    );
    // The round retains its result before optional cache work publishes it.
    assert_in_order(
        execution,
        "package review",
        &["round.retain(", "cache_write.component_source()"],
    );
}

#[test]
fn package_commands_schedule_work_outside_package_policy() {
    assert_in_order(
        include_str!("package_commands.rs"),
        "package commands",
        &[
            "PackageCommandTransaction::open(options)",
            "transaction.prepare(command, None)",
            "resolve_command_sources(preparation, session)",
            "for request in prepared.review_requests()",
            "request.review(",
            "prepared.complete(reviews, &mut session.packages)",
        ],
    );
    let policy = include_str!("../../../packages/package-manager/src/package_manager.rs");
    assert_absent(
        &[policy],
        "package policy schedules",
        &["execute_package_command(", "CompilationSession"],
    );
}

#[test]
fn application_depends_on_facades_not_language_stages() {
    assert_absent(
        &[include_str!("../../cli/src/cli/compilation.rs")],
        "CLI reaches behind Build:",
        &["prepare_local_project_for_targets(", "publish_compilation("],
    );
    assert_absent(
        &[include_str!("../../cli/src/execution/compilation.rs")],
        "CLI reaches behind Build:",
        &["prepare_local_project(", "compiler::compile("],
    );
    assert_absent(
        &[include_str!("../../cli/src/inspection/mod.rs")],
        "CLI reaches behind Build:",
        &["prepare_local_project(", "CompilationSession::default()"],
    );
}

#[test]
fn source_and_package_execution_have_one_application_owner() {
    assert_absent(
        &[
            include_str!("../../../compilation/driver/src/compiler.rs"),
            include_str!("../../../compilation/driver/src/checked/checking.rs"),
            include_str!("../../../compilation/driver/src/checked/compilation_session.rs"),
        ],
        "compiler executes Build:",
        &[
            ".host()",
            "host.start(",
            "activation.advance(",
            "compile_with_session(",
        ],
    );
}

#[test]
fn application_completes_native_products_without_source_continuations() {
    let completion = include_str!("native_completion.rs");
    assert_in_order(
        completion,
        "native completion",
        &[
            "native_production::prepare(",
            "prepared.scalar_completions()",
            "build_evaluation::scalar_completion::complete(",
            "completion.complete(completed)",
            "native_production::compile(prepared)",
            ".finish_native(",
            ".with_runtime_provisions(",
        ],
    );
    assert_absent(
        &[completion],
        "native completion reopens",
        &[
            "CheckedCompilation",
            "CheckedTrees",
            "compile_with_session",
            ".select_build_batch(",
        ],
    );
}

#[test]
fn build_entrance_shows_the_ordered_project_lifecycle() {
    assert_in_order(
        include_str!("build.rs"),
        "Build entrance",
        &[
            "build_directory::admit_outputs(",
            "begin_local_project_preparation(",
            "crate::resolve_project_sources(",
            "products::compile(",
            "publication::publish_products(",
        ],
    );
}

#[test]
fn ordinary_package_operations_admit_instead_of_execute() {
    let execution = include_str!("project_compilation.rs");
    assert_in_order(
        execution,
        "project compilation",
        &[
            "package_manager::operations::admit_project_products(",
            "crate::package_compilation::compile_packages(",
            ".complete(products, &mut session.packages)",
        ],
    );
}

#[test]
fn application_owns_the_compiler_separately_from_package_resources() {
    let resources = include_str!("../../../packages/package-manager/src/build_session.rs");
    assert_absent(
        &[resources],
        "package resources own the compiler:",
        &["CompilationSession", "fn compiler("],
    );
    assert_absent(
        &[
            include_str!("package_commands.rs"),
            include_str!("package_inspection.rs"),
            include_str!("project_compilation.rs"),
            include_str!("build_execution/source_resolution.rs"),
        ],
        "application entrance builds its own compiler:",
        &["CompilationSession::default()"],
    );
}

#[test]
fn application_schedules_dependencies_without_opening_source_continuations() {
    assert_absent(
        &[
            include_str!("build_execution.rs"),
            include_str!("package_compilation.rs"),
            include_str!("build_execution/source_resolution.rs"),
            include_str!("build_execution/source_resolution/readiness.rs"),
        ],
        "application opens source:",
        &[
            "CheckedCompilation",
            "PreparedPackageBuild",
            ".into_continuation(",
            ".select_build_batch(",
            "produce_products(",
        ],
    );
    assert_absent(
        &[include_str!(
            "../../../compilation/driver/src/checked/package_compilation.rs"
        )],
        "compiler still schedules",
        &[
            "fn compile_packages(",
            "fn check_package(",
            "fn join_dependency_builds(",
            "PackageFrontier",
            "join_generated_sources(",
        ],
    );
}
