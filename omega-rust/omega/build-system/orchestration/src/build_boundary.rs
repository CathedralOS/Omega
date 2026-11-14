/*

Source-level ownership and call-order checks for the orchestration boundary.

Crate-level dependency edges are checked by `cargo xtask check-boundary` from
`cargo metadata`. These tests check the finer-grained division of labor *within*
the crates that legitimately depend on one another:
- `readiness.rs` drives dependency and helper-product work iteratively on its
  explicit work stack rather than through recursive Rust calls.
- `package_inspection`, `package_commands`, `package_review`, `native_completion`,
  `build.rs`, and `project_compilation` execute their lifecycle steps in canonical
  order while `package-manager` and `compiler` remain free of cross-layer
  scheduling calls.
- `assert_in_order` strips whitespace before matching so `rustfmt` line-breaks
  never cause false failures.

*/

fn assert_absent(sources: &[&str], complaint: &str, tokens: &[&str]) {
    for source in sources {
        for token in tokens {
            assert!(!source.contains(token), "{complaint} {token}");
        }
    }
}

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
    let source = include_str!("compile/build_execution/source_resolution/readiness.rs");
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
    let source = include_str!("compile/build_execution/source_resolution/readiness.rs");
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
        include_str!("package/package_inspection.rs"),
        "package audit",
        &[
            "PackageInspectionTransaction::open(options)",
            "BuildSession::create_retained(",
            "transaction.prepare(None)",
            "resolve_inspection_sources(&inspection, &mut session)",
            "for target in inspection.targets().to_vec()",
            "inspection.review_target(",
            "inspection.finish()",
            "session.dispose_inspection(result)",
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
            ".begin_package_build(",
            ".complete_package_build(",
            ".review_package_build(",
            ".publish_component(",
            "PendingReviewBuild",
        ],
    );
    let execution = include_str!("package/package_review.rs");
    assert_absent(
        &[execution],
        "application bypasses the review round:",
        &["execute_package_build("],
    );
    assert_in_order(
        execution,
        "package review",
        &["round.retain(", "cache_write.component_source()"],
    );
}

#[test]
fn package_commands_schedule_work_outside_package_policy() {
    assert_in_order(
        include_str!("package/package_commands.rs"),
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
        &[include_str!(
            "../../../application/cli/src/commands/compile.rs"
        )],
        "CLI reaches behind Build:",
        &["publish_compilation("],
    );
    assert_absent(
        &[include_str!(
            "../../../application/cli/src/execution/compilation.rs"
        )],
        "CLI reaches behind Build:",
        &["compiler::compile("],
    );
    assert_absent(
        &[include_str!(
            "../../../application/cli/src/inspection/mod.rs"
        )],
        "CLI reaches behind Build:",
        &["CompilationSession::default()"],
    );
}

#[test]
fn source_and_package_execution_have_one_application_owner() {
    assert_absent(
        &[
            include_str!("../../../compilation/compiler/src/target_batch.rs"),
            include_str!("../../../compilation/compiler/src/checked/checking.rs"),
            include_str!("../../../compilation/compiler/src/checked/compilation_session.rs"),
        ],
        "compiler executes Build:",
        &[".host()", "host.start(", "activation.advance("],
    );
}

#[test]
fn application_completes_native_products_without_source_continuations() {
    let completion = include_str!("compile/native_completion.rs");
    assert_in_order(
        completion,
        "native completion",
        &[
            "native_compiler::prepare(",
            "prepared.scalar_completions()",
            "build_settings::scalar_completion::complete(",
            "completion.complete(completed)",
            "native_compiler::compile(prepared)",
            ".finish_native(",
            ".with_runtime_provisions(",
        ],
    );
    assert_absent(
        &[completion],
        "native completion reopens",
        &["CheckedCompilation", "CheckedTrees", ".select_build_batch("],
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
    let execution = include_str!("project/project_compilation.rs");
    assert_in_order(
        execution,
        "project compilation",
        &[
            "package_manager::operations::admit_project_products(",
            "crate::package::package_compilation::compile_packages(",
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
            include_str!("package/package_commands.rs"),
            include_str!("package/package_inspection.rs"),
            include_str!("project/project_compilation.rs"),
            include_str!("compile/build_execution/source_resolution.rs"),
        ],
        "application entrance builds its own compiler:",
        &["CompilationSession::default()"],
    );
}

#[test]
fn application_schedules_dependencies_without_opening_source_continuations() {
    assert_absent(
        &[
            include_str!("compile/build_execution.rs"),
            include_str!("package/package_compilation.rs"),
            include_str!("compile/build_execution/source_resolution.rs"),
            include_str!("compile/build_execution/source_resolution/readiness.rs"),
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
}
