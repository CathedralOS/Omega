//! A pure Build cache hit owes the current occurrence its recorded logical charge.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use artifacts::compile_timings::counters;
use build_time_evaluation::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};
use compiler::{CheckedCompileRequest, CompilationSession};

fn limits(fuel: u64) -> BuildEvaluationSponsorLimits {
    BuildEvaluationSponsorLimits::new(
        fuel, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
    )
    .expect("positive limits")
}

fn sponsor(limits: BuildEvaluationSponsorLimits) -> BuildEvaluationSponsor {
    BuildEvaluationSponsor::new_with_measurement(
        limits,
        evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .expect("current Terminal measurement")
}

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    initial_build_source_isolation();
    deferred_product_uses_captured_imports();
    unbound_build_invocation();
    interleaved_build_invocations();
    foreign_build_return_refuses();
    ignored_build_request_still_requires_join();
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some("linux_x86_64"),
    )
    .expect("fixture admission");
    let check = |session: &mut CompilationSession, sponsor: &BuildEvaluationSponsor, name| {
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(name));
        request.package_inputs = prepared.package_inputs.clone();
        request.build_execution_profile = target::TargetProfile::host_if_supported();
        request.build_snapshot =
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(
                request.package_inputs.as_ref(),
            );
        request.evaluation_sponsor = Some(sponsor.clone());
        session.check(request)
    };
    let measured = sponsor(limits(1_000_000));
    check(
        &mut CompilationSession::default(),
        &measured,
        "linux_x86_64",
    )
    .result
    .expect("cold calibration");
    let fuel = measured.consumed_fuel_units();
    let cells = measured.consumed_result_storage_cells();
    assert!(fuel > 1 && cells > 0);
    println!("one_build_fuel={fuel} result_cells={cells}");

    let mut session = CompilationSession::default();
    let shared_sponsor = sponsor(limits(2 * fuel));
    let before = counters::build_execution_count();
    check(&mut session, &shared_sponsor, "windows_x86_64")
        .result
        .expect("first row");
    check(&mut session, &shared_sponsor, "linux_x86_64")
        .result
        .expect("second row");
    let executions = counters::build_execution_count() - before;
    println!(
        "two_rows_executions={executions} charged_fuel={} charged_cells={}",
        shared_sponsor.consumed_fuel_units(),
        shared_sponsor.consumed_result_storage_cells()
    );
    assert_eq!(executions, 1);
    assert_eq!(
        shared_sponsor.consumed_fuel_units(),
        2 * fuel,
        "warm occurrence owes its logical fuel"
    );
    assert_eq!(shared_sponsor.consumed_result_storage_cells(), 2 * cells);
    let fresh = sponsor(limits(2 * fuel));
    check(&mut session, &fresh, "linux_x86_64")
        .result
        .expect("fresh account");
    assert_eq!(fresh.consumed_fuel_units(), fuel);
    assert_eq!(fresh.consumed_result_storage_cells(), cells);
    assert_eq!(counters::build_execution_count() - before, 1);
    println!("fresh_account_charged_without_execution");

    let mut limited_session = CompilationSession::default();
    let limited = sponsor(limits(2 * fuel - 1));
    check(&mut limited_session, &limited, "windows_x86_64")
        .result
        .expect("one affordable row");
    let before_refusal = counters::build_execution_count();
    let failure = check(&mut limited_session, &limited, "linux_x86_64")
        .result
        .expect_err("a warm hit cannot borrow its producer's allowance");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("aggregate fuel sponsor exhausted")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count(), before_refusal);
    assert_eq!(
        limited.consumed_fuel_units(),
        fuel,
        "refused replay charge is atomic"
    );
    assert_eq!(limited.consumed_result_storage_cells(), cells);
    println!("warm_limit_rejected_without_execution");

    // Force a cold second occurrence with the same remaining allowance. Both
    // policies reject; only the cold path performs another actual execution.
    let cold = sponsor(limits(2 * fuel - 1));
    check(&mut CompilationSession::default(), &cold, "windows_x86_64")
        .result
        .expect("affordable cold row");
    let before_cold = counters::build_execution_count();
    let failure = check(&mut CompilationSession::default(), &cold, "linux_x86_64")
        .result
        .expect_err("cold occurrence must reject the same allowance");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("evaluation Fuel allowance exhausted")
        }),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count() - before_cold, 1);
    println!("cold_and_warm_hard_ceiling_decisions_agree");

    // A later-axis refusal must not partially spend fuel on the cached path.
    let result_limits = BuildEvaluationSponsorLimits::new(
        2 * fuel,
        1_000_000,
        1_000_000,
        1_000_000,
        1_000_000,
        1_000_000,
        2 * cells - 1,
        1_000_000,
    )
    .expect("positive result ceiling");
    let result_limited = sponsor(result_limits);
    let mut result_session = CompilationSession::default();
    check(&mut result_session, &result_limited, "windows_x86_64")
        .result
        .expect("one affordable result");
    let before_result = counters::build_execution_count();
    let failure = check(&mut result_session, &result_limited, "linux_x86_64")
        .result
        .expect_err("warm result custody must be paid");
    assert!(
        failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("result-cell sponsor exhausted"))
    );
    assert_eq!(counters::build_execution_count(), before_result);
    assert_eq!(result_limited.consumed_fuel_units(), fuel);
    assert_eq!(result_limited.consumed_result_storage_cells(), cells);
    println!("warm_result_limit_rejected_atomically");

    logged_build_reuse();
    anonymous_build_completes_before_product();
    sliced_build_retains_effects();
}

/// Preparing the Build must not load the Product root or resolve its imports.
/// Both phases still consume the same closed source capture.
fn initial_build_source_isolation() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/separate_build_product_imports_exit/main.omg");
    let mut timings = artifacts::compile_timings::CompileTimings::default();
    let checkpoint = package_compilation::source_assembly::ImmutableSourceParseCheckpoint::prepare(
        &root,
        None,
        None,
        &mut timings,
    )
    .expect("captured Build sources");
    let (_, mut syntax) = checkpoint
        .assemble(None, &mut timings)
        .expect("Build assembly");
    assert!(syntax.build_source_id.is_some());
    assert!(
        syntax
            .sources
            .files()
            .any(|file| file.path == root.with_file_name("naming.omg")
                && file.dependency_scope == source::DependencyScope::Build)
    );
    assert!(
        syntax
            .sources
            .files()
            .all(|file| file.path != root.with_file_name("result.omg"))
    );
    assert!(
        syntax.sources.files().all(|file| file.path != root),
        "initial Build preparation loaded the Product root"
    );
    println!("initial_build_did_not_load_product_root");
    let original = syntax.sources.get(syntax.build_source_id.unwrap()).unwrap();
    let (_, product) = syntax
        .product_sources
        .take()
        .expect("pending Product sources")
        .assemble(None, &mut timings)
        .expect("joined Product sources");
    assert!(product.sources.files().any(|file| file.path == root));
    assert!(
        product
            .sources
            .files()
            .any(|file| file.path == root.with_file_name("result.omg")
                && file.dependency_scope == source::DependencyScope::Product)
    );
    let retained = product.sources.get(original.source_id).unwrap();
    assert_eq!(retained.path, original.path);
    assert!(std::sync::Arc::ptr_eq(&retained.source, &original.source));
    assert!(product.product_sources.is_none());
    for sources in [&syntax.sources, &product.sources] {
        assert!(
            sources
                .files()
                .any(|file| file.path.ends_with("float_operations.omg")
                    && file.dependency_scope == source::DependencyScope::Build)
        );
    }
    println!("product_preparation_retained_the_exact_build_source_bytes");
}

/// An import discovered after Build still resolves in the captured namespace,
/// including existence/canonicalization, not just the final file-byte read.
fn deferred_product_uses_captured_imports() {
    let authored = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/separate_build_product_imports_exit");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let temporary = std::env::temp_dir().join(format!(
        "omega-build-capture-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&temporary).expect("exclusive temporary source root");
    for name in ["main.omg", "build.omg", "naming.omg", "result.omg"] {
        std::fs::copy(authored.join(name), temporary.join(name)).expect("copy authored source");
    }
    let root = std::fs::canonicalize(&temporary).expect("temporary root");
    let mut timings = artifacts::compile_timings::CompileTimings::default();
    let checkpoint = package_compilation::source_assembly::ImmutableSourceParseCheckpoint::prepare(
        &root,
        None,
        None,
        &mut timings,
    )
    .expect("directory-root Build capture");
    let (_, build) = checkpoint
        .assemble(None, &mut timings)
        .expect("Build assembly");
    std::fs::remove_file(root.join("result.omg")).expect("remove only the live Product helper");
    let (_, product) = build
        .product_sources
        .expect("deferred Product")
        .assemble(None, &mut timings)
        .expect("Product import resolves from capture after live deletion");
    assert!(
        product
            .sources
            .files()
            .any(|file| file.path == root.join("result.omg"))
    );
    std::fs::remove_dir_all(&root).expect("remove exclusive temporary fixture");
    println!("deferred_product_import_used_captured_namespace_after_live_deletion");
}

fn unbound_build_invocation() {
    let authored = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/separate_build_product_imports_exit");
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("unbound-build", false)
            .expect("exclusive source workspace");
    for name in ["main.omg", "build.omg", "naming.omg", "result.omg"] {
        std::fs::copy(authored.join(name), temporary.path().join(name))
            .expect("copy authored source");
    }
    let capture = package_source::local::operations::capture_local_source_input(
        temporary.path(),
        package_source::LocalSourceLimits::default(),
    )
    .expect("acquire source before its Build declares an owner");
    let root = capture.canonical_live_root().join("main.omg");
    for name in ["main.omg", "build.omg", "naming.omg", "result.omg"] {
        std::fs::remove_file(temporary.path().join(name))
            .expect("remove live copy after acquisition");
    }
    let request = CheckedCompileRequest {
        source_capture: Some(capture.clone()),
        ..CheckedCompileRequest::new(
            &root,
            Some(
                target::TargetProfile::host_if_supported()
                    .expect("host target")
                    .target_name(),
            ),
        )
    };
    let mut compiler = CompilationSession::default();
    let before = counters::build_execution_count();
    let mut invocation = compiler
        .begin_package_build(request)
        .unwrap_or_else(|failure| {
            panic!(
                "Build must precede its declared package identity: {:?}",
                failure.into_diagnostics()
            )
        });
    let returned = return_build(&mut invocation);
    let inputs = unbound_join_inputs(
        &capture,
        capture.tree().clone(),
        "separate_build_product_imports_exit",
    );
    invocation
        .join_package_inputs(inputs)
        .expect("join exact source owner");
    let build = compiler
        .complete_package_build(invocation, returned)
        .unwrap_or_else(|failure| panic!("unbound completion: {:?}", failure.into_diagnostics()));
    let product = compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut outcomes = compiler
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product,
                permissions: None,
            },
        )])
        .expect("joined Product batch");
    let (_, outcome) = outcomes.pop().expect("one Product outcome");
    let report = outcome
        .and_then(compiler::CompilationOutput::into_report)
        .unwrap_or_else(|error| panic!("joined native Product: {:?}", error.into_diagnostics()));
    let artifact = report
        .retained_native_artifact()
        .expect("native product")
        .psi_artifact();
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("named Product publishes portable Terminal");
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        x86_flags: None,
        provider_proposal: None,
        artifact: &reloaded,
        target: Some(
            target::TargetProfile::host_if_supported()
                .unwrap()
                .target_name(),
        ),
        provisions: report.runtime_provisions(),
        fuel_allowance: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .expect("execute joined Terminal Product");
    assert!(
        matches!(
            observation.status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "{:?}",
        observation.status
    );
    let output = temporary.path().join("output");
    let published = compilation_products::publication::publish_compilation(report, &output)
        .expect("publish native Product");
    let observed = std::process::Command::new(
        published
            .checked_native_executable_path()
            .expect("native executable"),
    )
    .output()
    .expect("execute joined native Product");
    assert_eq!(observed.status.code(), Some(0), "{observed:?}");
    assert_eq!(counters::build_execution_count(), before + 1);
    println!(
        "acquired_unbound_build_joined_named_product_native_and_terminal_exit0_without_live_source_or_replay"
    );

    let other = package_source::local::operations::capture_local_source_input(
        fixture_package_inputs::repo_root()
            .join("tests/omega/pass/build/activation_yields_without_replay"),
        package_source::LocalSourceLimits::default(),
    )
    .expect("different acquired namespace for refusal control");
    let wrong_capture = unbound_join_inputs(
        &capture,
        other.tree().clone(),
        "separate_build_product_imports_exit",
    );
    let wrong_name = unbound_join_inputs(&capture, capture.tree().clone(), "wrong_name");
    let wrong_purpose = unbound_join_inputs(
        &capture,
        capture.tree().clone(),
        "separate_build_product_imports_exit",
    )
    .with_compilation_purpose(build_declarations::DependencyPurpose::Build)
    .expect("Build-purpose control");
    let root_owner = fixture_package_inputs::fixture_package_identity(71);
    let dependency_owner = fixture_package_inputs::fixture_package_identity(72);
    let extra_dependency = package_compilation::PackageCompilationInputs::new(
        root_owner,
        build_declarations::BuildDeclarationKind::Application,
        vec![
            package_compilation::PackageSourceBinding::new(
                root_owner,
                "separate_build_product_imports_exit",
                capture.canonical_live_root().to_path_buf(),
            )
            .with_captured_source(capture.tree().clone())
            .expect("root custody"),
            package_compilation::PackageSourceBinding::new(
                dependency_owner,
                "unrequested",
                other.canonical_live_root().to_path_buf(),
            ),
        ],
        vec![package_compilation::PackageDependencyBinding::new(
            root_owner,
            "unrequested",
            dependency_owner,
        )],
    )
    .expect("well-formed but unrequested graph");
    for (inputs, fragment) in [
        (
            Some(wrong_capture),
            "joined package source differs from the original Build capture",
        ),
        (
            Some(wrong_name),
            "joined package identity does not match the executed Build declaration",
        ),
        (
            Some(extra_dependency),
            "joined package dependencies require executed-request settlement",
        ),
        (
            Some(wrong_purpose),
            "build-purpose package compilation requires its target to equal the admitted build execution profile",
        ),
        (
            None,
            "Build completion requires joined package source custody",
        ),
    ] {
        let mut session = CompilationSession::default();
        let mut invocation = session
            .begin_package_build(CheckedCompileRequest {
                source_capture: Some(capture.clone()),
                build_execution_profile: Some(
                    target::TargetProfile::from_omega_target_name(Some("macos_arm64"))
                        .expect("control Build profile"),
                ),
                ..CheckedCompileRequest::new(&root, Some("linux_x86_64"))
            })
            .expect("admit same unbound source for control");
        let returned = return_build(&mut invocation);
        if let Some(inputs) = inputs {
            invocation
                .join_package_inputs(inputs)
                .expect("supply control join");
        }
        let failure = session
            .complete_package_build(invocation, returned)
            .err()
            .expect("invalid join must refuse");
        let diagnostics = failure.into_diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(fragment)),
            "{diagnostics:?}"
        );
    }
    println!(
        "unbound_join_rejected_changed_capture_wrong_declaration_unrequested_dependency_and_missing_custody"
    );
    let mixed = CheckedCompileRequest {
        source_capture: Some(capture.clone()),
        package_inputs: Some(unbound_join_inputs(
            &capture,
            capture.tree().clone(),
            "separate_build_product_imports_exit",
        )),
        ..CheckedCompileRequest::new(&root, Some("linux_x86_64"))
    };
    let before = counters::build_execution_count();
    let failure = CompilationSession::default()
        .begin_package_build(mixed)
        .err()
        .expect("mixed source modes refuse");
    assert!(
        failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "source preparation cannot combine an unbound capture with a supplied package graph"
            ))
    );
    assert_eq!(counters::build_execution_count(), before);
}

fn unbound_join_inputs(
    capture: &package_compilation::CapturedLocalSource,
    tree: std::sync::Arc<source::CapturedSourceTree>,
    name: &str,
) -> package_compilation::PackageCompilationInputs {
    let identity = fixture_package_inputs::fixture_package_identity(71);
    package_compilation::PackageCompilationInputs::new(
        identity,
        build_declarations::BuildDeclarationKind::Application,
        vec![
            package_compilation::PackageSourceBinding::new(
                identity,
                name,
                capture.canonical_live_root().to_path_buf(),
            )
            .with_captured_source(tree)
            .expect("retained source binding"),
        ],
        Vec::new(),
    )
    .expect("final owner supplied after Build return")
}

fn package_invocation_requests(fixture: &str) -> impl Fn() -> CheckedCompileRequest {
    let repository = fixture_package_inputs::repo_root();
    let authored = repository.join("tests/omega").join(fixture);
    let captured = fixture_package_inputs::sealed_fixture_staging::stage_sealed_snapshot(
        &repository,
        &authored,
    );
    // This SDK owns supplied source custody. Do not route it through static
    // Build-role extraction: the witness deliberately has an unselected helper
    // with a duplicate role, which only execution can distinguish.
    let identity = fixture_package_inputs::fixture_package_identity(1);
    let binding = package_compilation::PackageSourceBinding::new(
        identity,
        authored.file_name().unwrap().to_str().unwrap(),
        captured.clone(),
    )
    .with_canonical_source_metadata()
    .expect("sealed fixture custody");
    let inputs = package_compilation::PackageCompilationInputs::new(
        identity,
        build_declarations::BuildDeclarationKind::Application,
        vec![binding],
        Vec::new(),
    )
    .expect("supplied source owner");
    move || CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&captured.join("main.omg"), Some("linux_x86_64"))
    }
}

fn return_build(
    invocation: &mut compiler::PackageBuildInvocation,
) -> Option<build_evaluation::ReturnedBuild> {
    invocation.execution().map(|execution| {
        let mut host = execution.host().expect("issue Build host once");
        let mut activation = host.start().expect("start Build once");
        while !activation.advance(1024).expect("advance Build").returned {}
        activation.complete().expect("normal Build return")
    })
}

fn interleaved_build_invocations() {
    let request = package_invocation_requests("pass/build/activation_yields_without_replay");
    let mut session = CompilationSession::default();
    let before = counters::build_execution_count();
    let products_before = counters::product_source_completion_count();
    let mut first = session
        .begin_package_build(request())
        .expect("first admission");
    assert_eq!(counters::build_execution_count(), before);
    assert_eq!(counters::product_source_completion_count(), products_before);
    let first_returned = {
        let mut host = first
            .execution()
            .expect("fresh Build")
            .host()
            .expect("first host");
        let mut activation = host.start().expect("first activation");
        let step = activation.advance(64).expect("first partial execution");
        assert!(!step.returned);
        assert_eq!(
            step.requests.len(),
            1,
            "early role is observable before return"
        );
        // The first activation borrows no compiler session. Finish another
        // package through Product checking while its continuation stays live.
        let mut second = session
            .begin_package_build(request())
            .expect("second admission while paused");
        assert_eq!(counters::build_execution_count(), before + 1);
        let second_returned = return_build(&mut second);
        let second = session
            .complete_package_build(second, second_returned)
            .expect("second completion");
        session
            .check_package_build(second)
            .expect("second Product while first is paused");
        while !activation.advance(1024).expect("resume first").returned {}
        activation.complete().expect("first normal return")
    };
    assert_eq!(
        first_returned.requests().len(),
        1,
        "stepping cannot erase earlier requests"
    );
    assert!(
        first
            .execution()
            .expect("retained invocation")
            .host()
            .is_err(),
        "host replacement cannot replay Build"
    );
    let first = session
        .complete_package_build(first, Some(first_returned))
        .expect("first completion");
    session.check_package_build(first).expect("first Product");
    assert_eq!(counters::build_execution_count(), before + 2);
    println!("paused_build_survived_another_package_product_without_replay");
}

fn foreign_build_return_refuses() {
    let request = package_invocation_requests("pass/build/activation_yields_without_replay");
    let mut session = CompilationSession::default();
    let mut first = session
        .begin_package_build(request())
        .expect("first admission");
    let mut second = session
        .begin_package_build(request())
        .expect("second admission");
    let first_returned = return_build(&mut first);
    let second_returned = return_build(&mut second);
    for (invocation, returned) in [(first, second_returned), (second, first_returned)] {
        let failure = session
            .complete_package_build(invocation, returned)
            .err()
            .expect("foreign completion refuses");
        assert!(failure.into_diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("completion belongs to another invocation")
        }));
    }
    println!("equal_code_builds_rejected_swapped_normal_returns");
}

fn ignored_build_request_still_requires_join() {
    let request = package_invocation_requests("fail/build/unjoined_executed_dependency");
    let mut session = CompilationSession::default();
    let mut invocation = session
        .begin_package_build(request())
        .expect("Build admission does not need the requested Product dependency");
    // This helper intentionally ignores each step's request view.
    let returned = return_build(&mut invocation).expect("fresh Build return");
    assert_eq!(
        returned
            .requests()
            .iter()
            .filter(|issued| matches!(
                issued.request(),
                build_declarations::BuildRequest::Dependency { .. }
            ))
            .count(),
        1
    );
    let failure = session
        .complete_package_build(invocation, Some(returned))
        .err()
        .expect("mandatory join");
    assert!(
        failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "dependency requests require coordinator settlement before Product compilation"
            ))
    );
    println!("ignored_step_request_remained_in_mandatory_dependency_join");
}

/// The ordinary source call crosses the production scheduling quantum. Its
/// early project declaration and late entry bindings must each execute once.
fn sliced_build_retains_effects() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/activation_yields_without_replay/main.omg");
    let account = sponsor(limits(1_000_000));
    let mut request = CheckedCompileRequest::new(&root, Some("linux_x86_64"));
    request.evaluation_sponsor = Some(account.clone());
    let before = counters::build_execution_count();
    CompilationSession::default()
        .check(request)
        .result
        .expect("sliced Build completes");
    assert_eq!(counters::build_execution_count() - before, 1);
    let fuel = account.consumed_fuel_units();
    assert!(
        fuel > 1024,
        "witness must cross the production scheduling slice: {fuel}"
    );
    assert_eq!(account.live_storage_cells(), 0);
    assert_eq!(account.live_payload_bytes(), 0);
    println!("sliced_build_executed_once_with_fuel={fuel}");
    let limited = sponsor(limits(fuel - 1));
    let mut request = CheckedCompileRequest::new(&root, Some("linux_x86_64"));
    request.evaluation_sponsor = Some(limited.clone());
    let before = counters::build_execution_count();
    let failure = CompilationSession::default()
        .check(request)
        .result
        .expect_err("slice replenishment must not enlarge the invocation allowance");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("evaluation Fuel allowance exhausted")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count() - before, 1);
    assert_eq!(limited.live_storage_cells(), 0);
    assert_eq!(limited.live_payload_bytes(), 0);
    println!("sliced_build_short_allowance_refused_and_storage_retired");
}

/// Product ownership requires another source context, not another activation.
/// Two requested targets consume the same completed anonymous Build.
fn anonymous_build_completes_before_product() {
    let repository = fixture_package_inputs::repo_root();
    let root = repository.join("tests/omega/pass/build/terminal_project_requests/main.omg");
    let targets = ["windows_x86_64", "linux_x86_64"]
        .into_iter()
        .map(|name| Some(target::TargetProfile::from_omega_target_name(Some(name)).unwrap()))
        .collect();
    let request = compiler::CompileRequest::for_targets(
        root,
        repository.join("build/package-build-owner/anonymous-once"),
        targets,
    )
    .with_requested_product(compiler::RequestedCompileProduct::TerminalArtifact);
    let before = counters::build_execution_count();
    let outcomes = compiler::compile(request).expect("anonymous multi-target request");
    assert_eq!(outcomes.outcomes().len(), 2);
    let mut identity = None;
    for outcome in outcomes.into_outcomes() {
        let report = outcome
            .into_result()
            .expect("anonymous Product publication");
        let artifact = report.artifact().expect("portable Terminal product");
        assert!(
            artifact
                .semantic_module()
                .identities
                .invocation_scope()
                .is_none()
        );
        let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
            .expect("anonymous Product reload");
        assert_eq!(artifact.view().identity(), reloaded.view().identity());
        match identity {
            Some(previous) => assert_eq!(previous, artifact.view().identity()),
            None => identity = Some(artifact.view().identity()),
        }
    }
    assert_eq!(counters::build_execution_count() - before, 1);
    println!("anonymous_build_executed_once_before_two_portable_products");
}

/// The real source fixture writes through its original log facet. Both target
/// occurrences must retain the exact bytes, but only one executes guest code.
fn logged_build_reuse() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/build_log_line_written/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some("linux_x86_64"),
    )
    .expect("logging fixture admission");
    let expected = b"omega build log witness\n";
    let check = |session: &mut CompilationSession, account: &BuildEvaluationSponsor, target| {
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(target));
        request.package_inputs = prepared.package_inputs.clone();
        request.build_execution_profile = target::TargetProfile::host_if_supported();
        request.build_snapshot =
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(
                request.package_inputs.as_ref(),
            );
        request.evaluation_sponsor = Some(account.clone());
        session.check(request)
    };
    let mut session = CompilationSession::default();
    let account = sponsor(limits(1_000_000));
    let before = counters::build_execution_count();
    let mut cold_fuel = 0;
    for target in ["windows_x86_64", "linux_x86_64"] {
        let checked = check(&mut session, &account, target)
            .result
            .expect("logged Build");
        let build = checked.build();
        assert_eq!(
            build
                .observation_summary
                .as_ref()
                .expect("settled observations")
                .build_log(),
            expected
        );
        let usage = build.evaluation_usage.expect("charged execution");
        assert_eq!(usage.build_log_bytes, expected.len() as u64);
        assert!(usage.result_payload_bytes >= expected.len() as u64);
        if cold_fuel == 0 {
            cold_fuel = usage.fuel_units;
        } else {
            assert_eq!(usage.fuel_units, cold_fuel);
        }
    }
    assert_eq!(counters::build_execution_count() - before, 1);
    assert_eq!(account.consumed_fuel_units(), 2 * cold_fuel);
    assert_eq!(account.live_storage_cells(), 0);
    assert_eq!(account.live_payload_bytes(), 0);
    println!("logged_build_reused_once_with_exact_bytes_and_paid_custody");

    // One byte short of two logs: a warm occurrence must refuse before
    // spending fuel or publishing the cached output as a successful result.
    let limited = sponsor(
        BuildEvaluationSponsorLimits::new(
            1_000_000,
            2 * expected.len() as u64 - 1,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
        )
        .expect("positive log ceiling"),
    );
    let mut limited_session = CompilationSession::default();
    check(&mut limited_session, &limited, "windows_x86_64")
        .result
        .expect("one affordable log");
    let before_refusal = counters::build_execution_count();
    let failure = check(&mut limited_session, &limited, "linux_x86_64")
        .result
        .expect_err("cached log still costs bytes");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("aggregate BuildLog sponsor exhausted")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count(), before_refusal);
    assert_eq!(limited.consumed_fuel_units(), cold_fuel);
    println!("warm_log_limit_rejected_atomically_without_execution");

    let before_cold = counters::build_execution_count();
    let failure = check(&mut CompilationSession::default(), &limited, "linux_x86_64")
        .result
        .expect_err("executed logging must reject the same remaining byte allowance");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("evaluation BuildLog allowance exhausted")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count() - before_cold, 1);
    assert_eq!(limited.live_storage_cells(), 0);
    assert_eq!(limited.live_payload_bytes(), 0);
    println!("cold_log_limit_rejected_and_live_storage_retired");
}
