//! A pure Build cache hit owes the current occurrence its recorded logical charge.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "../tests/support/joined_product_execution.rs"]
mod joined_product_execution;
use joined_product_execution::{execute_joined_product, execute_report};

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
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--anonymous-generator"))
    {
        anonymous_generator_source_scope();
        return;
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--source-inputs")) {
        source_discovery_root_inputs();
        return;
    }
    initial_build_source_isolation();
    deferred_product_uses_captured_imports();
    unbound_build_invocation("separate_build_product_imports_exit");
    unbound_build_invocation("executed_project_identity_exit");
    executed_dependency_source_join("executed_dependency_source_join_exit", false);
    executed_dependency_source_join("executed_dependency_source_join_exit", true);
    executed_dependency_source_join("executed_dependency_shared_source_exit", false);
    application_source_resolution("executed_dependency_source_join_exit", false);
    application_source_resolution("executed_dependency_source_join_exit", true);
    application_source_resolution("executed_dependency_shared_source_exit", false);
    project_session_lifecycle();
    completed_build_target_fanout();
    interleaved_build_invocations();
    terminal_build_filesystem();
    anonymous_generator_source_scope();
    source_discovery_root_inputs();
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

/// Discovery binds the complete root snapshot before its first effect.
fn source_discovery_root_inputs() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "discovery-root-inputs",
        false,
    )
    .unwrap();
    let repository = fixture_package_inputs::repo_root();
    let fixture = repository.join("tests/omega/pass/build/captured_named_input_required_output");
    let project = temporary.path().join("project");
    std::fs::create_dir(&project).unwrap();
    for name in ["main.omg", "build.omg", "input.txt", "package-only.txt"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    std::fs::copy(fixture.join("README.md"), project.join("unselected.txt")).unwrap();
    let template = fixture_package_inputs::sealed_fixture_staging::stage_sealed_snapshot(
        &repository,
        &fixture.join("build-inputs/template"),
    );
    let input = package_compilation::capture_package_source_input(&template).unwrap();
    let expected = std::fs::read(template.join("input.txt")).unwrap();
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let snapshot = |required| {
        let source = package_compilation::BuildSourceCaptureRequest::new(
            ["main.omg", "build.omg", "input.txt", "package-only.txt"].map(|name| {
                (
                    name.as_bytes().to_vec(),
                    package_compilation::BuildSourceCaptureObligation::Required,
                )
            }),
        )
        .unwrap();
        package_compilation::BuildSnapshotRequest::scoped(required, source)
            .with_inputs([(b"template".to_vec(), input.clone())])
            .unwrap()
    };
    let preparation = || {
        package_manager::operations::begin_local_project_preparation(
            &project.join("main.omg"),
            &[profile],
            true,
        )
        .unwrap()
        .unwrap()
    };

    // Absence cannot fall back to the root's same-named input.txt or acquire
    // an ambient host path for the slot.
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("absent")).unwrap();
    let error = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation: preparation(),
            execution_profile: profile,
            snapshot: None,
        },
    )
    .err()
    .expect("an unassigned named slot must reject");
    let omega_build::ProjectSourceResolutionError::Compilation(error) = error else {
        panic!("named-slot refusal must come from execution: {error:?}");
    };
    let diagnostics = error.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("named build input `template` is not assigned")),
        "{diagnostics:?}"
    );
    session.close().unwrap();

    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("assigned")).unwrap();
    let before = counters::build_execution_count();
    let mut resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation: preparation(),
            execution_profile: profile,
            snapshot: Some(snapshot(vec![b"named.txt".to_vec()])),
        },
    )
    .unwrap_or_else(|error| panic!("assigned input discovery: {error:?}"));
    let root = resolved.builds.pop().unwrap();
    assert!(resolved.builds.is_empty());
    let inputs = root.package_inputs().clone();
    let build = root.complete(session.compiler(), profile, inputs).unwrap();
    let checked = session.compiler().check_package_build(build).unwrap();
    let observations = checked.build().observation_summary.as_ref().unwrap();
    assert_eq!(
        observations
            .captured_source_inventory()
            .unwrap()
            .entry_count(),
        5
    );
    assert_eq!(observations.named_input_inventories().len(), 1);
    assert_eq!(observations.required_output_settlements().len(), 1);
    let output = observations
        .staged_output_tree()
        .unwrap()
        .sealed_entry(b"named.txt")
        .unwrap();
    let build_output::BuildStagedOutputEntryKind::File { bytes, .. } = output.kind() else {
        panic!("named output must be a file");
    };
    assert_eq!(bytes, expected.as_slice());
    assert_eq!(counters::build_execution_count() - before, 1);
    session.close().unwrap();

    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("required")).unwrap();
    let mut resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation: preparation(),
            execution_profile: profile,
            snapshot: Some(snapshot(vec![
                b"named.txt".to_vec(),
                b"missing.txt".to_vec(),
            ])),
        },
    )
    .unwrap();
    let root = resolved.builds.pop().unwrap();
    let inputs = root.package_inputs().clone();
    let error = root
        .complete(session.compiler(), profile, inputs)
        .err()
        .expect("caller-required output cannot disappear after discovery");
    let diagnostics = error.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("required build output `missing.txt`")
            && diagnostic.message.contains("omitted")),
        "{diagnostics:?}"
    );
    session.close().unwrap();
    println!(
        "source_discovery_binds_the_complete_root_snapshot_and_preserves_named_slot_and_output_controls"
    );
}

/// Acquired source supplies both the compiler namespace and Build's input
/// inventory before a package name exists. Neither execution nor Product
/// completion may reopen the live project after that acquisition.
fn anonymous_generator_source_scope() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "anonymous-build-generator",
        false,
    )
    .unwrap();
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/captured_source_generated_constant");
    let project = temporary.path().join("project");
    std::fs::create_dir(&project).unwrap();
    for name in ["main.omg", "build.omg", "asset.txt"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    std::fs::copy(fixture.join("README.md"), project.join("unselected.txt")).unwrap();
    let expected = std::fs::read(project.join("asset.txt")).unwrap();
    let root = project.join("main.omg");
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let preparation =
        package_manager::operations::begin_local_project_preparation(&root, &[profile], true)
            .unwrap()
            .unwrap();
    let acquired = preparation
        .acquire_root()
        .expect("acquire anonymous source");
    let capture = acquired.source_capture().clone();
    let entry = preparation.entry_path();
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("session")).unwrap();
    let request = session
        .source_build_request(
            acquired.source_capture(),
            &preparation.entry_path(),
            profile,
        )
        .unwrap();
    let before = counters::build_execution_count();
    let mut invocation = session
        .compiler()
        .begin_package_build(request)
        .unwrap_or_else(|error| {
            panic!(
                "anonymous generator admission: {:?}",
                error.into_diagnostics()
            )
        });
    for name in ["main.omg", "build.omg", "asset.txt"] {
        std::fs::remove_file(project.join(name)).unwrap();
    }
    std::fs::remove_file(project.join("unselected.txt")).unwrap();
    let mut returned = return_build(&mut invocation).expect("anonymous generator return");
    let declared = acquired
        .declare_build(&returned)
        .expect("declare executed application");
    let package_manager::resolution::source::ExecutedSourceResolution::Complete(resolved) =
        declared.resolve().unwrap()
    else {
        panic!("generator authored no dependency request");
    };
    let inputs = resolved.join_build(resolved.root(), &mut returned).unwrap();
    invocation
        .join_product_context(profile, inputs.clone())
        .unwrap();
    let build = session
        .compiler()
        .complete_package_build(invocation, Some(returned))
        .unwrap_or_else(|error| {
            panic!(
                "anonymous generator completion: {:?}",
                error.into_diagnostics()
            )
        });
    assert_eq!(build.generated_source_bundle().sources().len(), 1);
    assert_eq!(
        build.generated_source_bundle().sources()[0].bytes(),
        expected
    );
    let checked = session
        .compiler()
        .check_package_build(build)
        .unwrap_or_else(|error| panic!("generated Product source: {:?}", error.into_diagnostics()));
    let observations = checked.build().observation_summary.as_ref().unwrap();
    assert!(observations.captured_source_inventory().is_some());
    assert_eq!(observations.required_output_settlements().len(), 1);
    assert_eq!(observations.filesystem_operation_attempts().len(), 6);
    assert_eq!(counters::build_execution_count() - before, 1);

    // Explicit narrowing still uses the same acquired bytes, even after the
    // live files disappear. The requested root and its Build inputs remain
    // visible, but an unselected asset from the complete capture does not.
    let selection = package_compilation::BuildSourceCaptureRequest::new(
        ["main.omg", "build.omg", "asset.txt"].map(|name| {
            (
                name.as_bytes().to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            )
        }),
    )
    .unwrap();
    let mut scoped = session
        .source_build_request(&capture, &entry, profile)
        .unwrap();
    scoped.build_snapshot = Some(package_compilation::BuildSnapshotRequest::scoped(
        [b"generated.omg".to_vec()],
        selection,
    ));
    let mut invocation = session
        .compiler()
        .begin_package_build(scoped)
        .unwrap_or_else(|error| {
            panic!(
                "narrowed anonymous admission: {:?}",
                error.into_diagnostics()
            )
        });
    let returned = return_build(&mut invocation);
    invocation
        .join_product_context(profile, inputs.clone())
        .unwrap();
    let build = session
        .compiler()
        .complete_package_build(invocation, returned)
        .unwrap_or_else(|error| {
            panic!(
                "narrowed anonymous completion: {:?}",
                error.into_diagnostics()
            )
        });
    assert_eq!(
        build.generated_source_bundle().sources()[0].bytes(),
        expected
    );
    let checked = session.compiler().check_package_build(build).unwrap();
    assert_eq!(
        checked
            .build()
            .observation_summary
            .as_ref()
            .unwrap()
            .captured_source_inventory()
            .unwrap()
            .entry_count(),
        4
    );

    let before_refusal = counters::build_execution_count();
    let missing_build = package_compilation::BuildSourceCaptureRequest::new(
        ["main.omg", "asset.txt"].map(|name| {
            (
                name.as_bytes().to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            )
        }),
    )
    .unwrap();
    let mut omitted = session
        .source_build_request(&capture, &entry, profile)
        .unwrap();
    omitted.build_snapshot = Some(package_compilation::BuildSnapshotRequest::scoped(
        [],
        missing_build,
    ));
    let error = session
        .compiler()
        .begin_package_build(omitted)
        .err()
        .expect("already-consumed Build source cannot be omitted");
    let diagnostics = error.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("scoped source inventory omits required source member build.omg")),
        "{diagnostics:?}"
    );
    assert_eq!(
        counters::build_execution_count(),
        before_refusal,
        "narrowing refuses before execution"
    );

    let missing_root = package_compilation::BuildSourceCaptureRequest::new(
        ["build.omg", "asset.txt"].map(|name| {
            (
                name.as_bytes().to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            )
        }),
    )
    .unwrap();
    let mut omitted = session
        .source_build_request(&capture, &entry, profile)
        .unwrap();
    omitted.build_snapshot = Some(package_compilation::BuildSnapshotRequest::scoped(
        [],
        missing_root,
    ));
    let error = session
        .compiler()
        .begin_package_build(omitted)
        .err()
        .expect("the explicitly requested Product root remains required before it is loaded");
    let diagnostics = error.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("scoped source inventory omits required source member main.omg")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count(), before_refusal);

    let mut missing_output = session
        .source_build_request(&capture, &entry, profile)
        .unwrap();
    missing_output.build_snapshot = Some(package_compilation::BuildSnapshotRequest::new([
        b"missing.txt".to_vec(),
    ]));
    let mut invocation = session
        .compiler()
        .begin_package_build(missing_output)
        .unwrap();
    let returned = return_build(&mut invocation);
    invocation.join_product_context(profile, inputs).unwrap();
    let error = session
        .compiler()
        .complete_package_build(invocation, returned)
        .err()
        .expect("caller-required outputs survive anonymous discovery");
    let diagnostics = error.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("required build output `missing.txt`")
            && diagnostic.message.contains("omitted")),
        "{diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count(), before_refusal + 1);
    preparation.complete_source_resolution(resolved).unwrap();
    session.close().unwrap();
    println!(
        "anonymous_generator_uses_captured_assets_after_live_deletion_preserves_narrowing_and_required_output_refusals"
    );

    // Exercise the application coordinator, not only its individual facade
    // calls. The returned activation remains the one Product completes.
    for name in ["main.omg", "build.omg", "asset.txt"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let preparation =
        package_manager::operations::begin_local_project_preparation(&root, &[profile], true)
            .unwrap()
            .unwrap();
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("application-session"))
            .unwrap();
    let before = counters::build_execution_count();
    let mut resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation,
            execution_profile: profile,
            snapshot: None,
        },
    )
    .unwrap_or_else(|error| panic!("application generator discovery: {error:?}"));
    assert_eq!(resolved.builds.len(), 1);
    for name in ["main.omg", "build.omg", "asset.txt"] {
        std::fs::remove_file(project.join(name)).unwrap();
    }
    let root = resolved.builds.pop().unwrap();
    let inputs = root.package_inputs().clone();
    let build = root.complete(session.compiler(), profile, inputs).unwrap();
    assert_eq!(
        build.generated_source_bundle().sources()[0].bytes(),
        expected
    );
    session.compiler().check_package_build(build).unwrap();
    assert_eq!(counters::build_execution_count() - before, 1);
    session.close().unwrap();
    println!(
        "application_discovery_retains_the_anonymous_generator_through_product_without_replay"
    );
}

/// One source fixture crosses the complete Terminal filesystem protocol. A
/// sliced invocation must retain six actual operations, one sealed output and
/// its exact generated bytes; resource refusal precedes the first Output effect.
fn terminal_build_filesystem() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/captured_source_generated_constant/main.omg");
    let profile = target::TargetProfile::host_if_supported().expect("host profile");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some(profile.target_name()),
    )
    .expect("captured generator inputs");
    let request = || {
        let mut request =
            CheckedCompileRequest::new(&prepared.root_path, Some(profile.target_name()));
        request.package_inputs = prepared.package_inputs.clone();
        request.build_snapshot =
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(
                request.package_inputs.as_ref(),
            );
        request
    };
    let mut session = CompilationSession::default();
    let before = counters::build_execution_count();
    let mut invocation = session
        .begin_package_build(request())
        .expect("generator admission");
    let returned = {
        let execution = invocation.execution().expect("authored Build");
        let mut host = execution.host().expect("generator host");
        let mut activation = host.start().expect("generator activation");
        let mut pauses = 0;
        loop {
            if activation
                .advance(1)
                .expect("sliced filesystem execution")
                .returned
            {
                break;
            }
            pauses += 1;
        }
        assert!(pauses > 6, "suspension crosses filesystem effects");
        activation.complete().expect("sealed generator return")
    };
    let build = session
        .complete_package_build(invocation, Some(returned))
        .expect("output settlement");
    let generated = build.generated_source_bundle().sources();
    assert_eq!(generated.len(), 1);
    assert_eq!(generated[0].relative_path(), b"generated.omg");
    assert_eq!(
        generated[0].bytes(),
        include_bytes!(
            "../../../../../tests/omega/pass/build/captured_source_generated_constant/asset.txt"
        )
    );
    let checked = session
        .check_package_build(build)
        .expect("generated source joins Product");
    let observations = checked
        .build()
        .observation_summary
        .as_ref()
        .expect("actual observations");
    assert_eq!(
        observations
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        [2, 4, 8, 1, 5, 8]
    );
    assert_eq!(observations.required_output_settlements().len(), 1);
    assert!(observations.captured_source_inventory().is_some());
    assert_eq!(
        checked
            .build()
            .evaluation_usage
            .unwrap()
            .filesystem_operation_attempts,
        6
    );
    assert_eq!(counters::build_execution_count() - before, 1);

    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "terminal-filesystem-budget",
        false,
    )
    .unwrap();
    let staging = build_time_evaluation::BuildMachineFilesystemSponsor::create_private(
        &temporary.path().join("staging"),
        build_time_evaluation::BuildMachineFilesystemSponsorLimits::COMPILER_DEFAULT,
    )
    .unwrap();
    let output = staging.session_root().unwrap().join("output");
    let account = sponsor(
        BuildEvaluationSponsorLimits::new(
            1_000_000, 1_000_000, 3, 16, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
        )
        .unwrap(),
    );
    let mut limited = request();
    limited.evaluation_sponsor = Some(account);
    limited.filesystem_sponsor = Some(staging);
    limited.build_dir = Some(output.clone());
    let failure = session
        .check(limited)
        .result
        .expect_err("fourth filesystem operation exceeds sponsor");
    let diagnostics = failure.into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("FilesystemAttempts allowance exhausted")
        }),
        "{diagnostics:?}"
    );
    assert!(
        !output.join("generated.omg").exists(),
        "no Output file may be created after refused preflight"
    );
    println!(
        "terminal_build_generator_sliced_once_six_operations_sealed_source_and_pre_effect_budget_refusal"
    );
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

fn unbound_build_invocation(project_name: &str) {
    let authored = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build")
        .join(project_name);
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("unbound-build", false)
            .expect("exclusive source workspace");
    for name in ["main.omg", "build.omg", "naming.omg", "result.omg"] {
        std::fs::copy(authored.join(name), temporary.path().join(name))
            .expect("copy authored source");
    }
    let acquired = package_manager::resolution::source::AcquiredBuildSource::capture_local(
        temporary.path(),
        package_source::LocalSourceLimits::default(),
        package_source::ExternalSourceContext::derive(b"build-charge-sdk"),
    )
    .expect("acquire source before its Build declares an owner");
    let capture = acquired.source_capture().clone();
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
    let actual_return = returned.as_ref().expect("authored Build normal return");
    let binding = acquired
        .bind_returned_build(actual_return)
        .expect("package manager binds the executed name to acquired source lineage");
    assert_eq!(binding.canonical_name(), project_name);
    assert!(
        !actual_return.requests().iter().any(|issued| matches!(
            issued.request(),
            build_declarations::BuildRequest::Dependency { .. }
        )),
        "unselected helper must not issue its missing dependency"
    );
    let inputs = package_compilation::PackageCompilationInputs::new(
        binding.identity(),
        actual_return.declaration().kind(),
        vec![binding],
        Vec::new(),
    )
    .expect("named root inputs; this Build issued no dependencies");
    invocation
        .join_product_context(target::TargetProfile::host_if_supported().unwrap(), inputs)
        .expect("join exact source owner");
    let build = compiler
        .complete_package_build(invocation, returned)
        .unwrap_or_else(|failure| panic!("unbound completion: {:?}", failure.into_diagnostics()));
    execute_joined_product(&mut compiler, build, &temporary.path().join("output"))
        .unwrap_or_else(|failure| panic!("joined Product: {:?}", failure.into_diagnostics()));
    assert_eq!(counters::build_execution_count(), before + 1);
    println!(
        "{project_name}: acquired_unbound_build_joined_named_product_native_and_terminal_exit0_without_live_source_or_replay"
    );

    let other = package_source::local::operations::capture_local_source_input(
        fixture_package_inputs::repo_root()
            .join("tests/omega/pass/build/activation_yields_without_replay"),
        package_source::LocalSourceLimits::default(),
    )
    .expect("different acquired namespace for refusal control");
    let wrong_capture = unbound_join_inputs(&capture, other.tree().clone(), project_name);
    let wrong_name = unbound_join_inputs(&capture, capture.tree().clone(), "wrong_name");
    let wrong_purpose = unbound_join_inputs(&capture, capture.tree().clone(), project_name)
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
                project_name,
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
                .join_product_context(
                    target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap(),
                    inputs,
                )
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
            project_name,
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

fn executed_source_workspace(
    project: &str,
    private_call: bool,
) -> platform_custody::temporary_directory::TemporaryDirectory {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "executed-source-graph",
        false,
    )
    .expect("exclusive source graph workspace");
    let fixtures = fixture_package_inputs::repo_root().join("tests/omega/pass/build");
    for package in [project, "executed_dependency_source_library"] {
        std::fs::create_dir(temporary.path().join(package)).unwrap();
        for name in ["main.omg", "build.omg"] {
            std::fs::copy(
                fixtures.join(package).join(name),
                temporary.path().join(package).join(name),
            )
            .unwrap();
        }
    }
    if private_call {
        std::fs::copy(
            fixture_package_inputs::repo_root()
                .join("tests/omega/fail/build/executed_dependency_private_call/main.omg"),
            temporary.path().join(project).join("main.omg"),
        )
        .unwrap();
    }
    temporary
}

fn application_source_resolution(project: &str, private_call: bool) {
    let temporary = executed_source_workspace(project, private_call);
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let preparation = package_manager::operations::begin_local_project_preparation(
        &temporary.path().join(project).join("main.omg"),
        &[profile],
        true,
    )
    .unwrap()
    .unwrap();
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("session"))
            .expect("one project resource owner");
    let sponsor = session.evaluation_sponsor().clone();
    let before = counters::build_execution_count();
    let resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation,
            execution_profile: profile,
            snapshot: None,
        },
    )
    .unwrap_or_else(|error| panic!("application source resolution: {error:?}"));
    assert_eq!(resolved.builds.len(), 2);
    assert_eq!(counters::build_execution_count(), before + 2);
    let discovery_fuel = sponsor.consumed_fuel_units();
    assert!(
        discovery_fuel > 0,
        "discovery must charge the project sponsor"
    );
    // Normal application discovery owns both captured namespaces and returned
    // activations. No later compiler/review consumer may need live source.
    for package in [project, "executed_dependency_source_library"] {
        for name in ["main.omg", "build.omg"] {
            std::fs::remove_file(temporary.path().join(package).join(name)).unwrap();
        }
    }
    let mut builds = resolved.builds;
    // Product placement is chosen after discovery. Its dependency bundles
    // carry that target, while the Build execution profile remains the host.
    let linux = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let product_target = if profile == linux {
        target::TargetProfile::from_omega_target_name(Some("windows_x86_64")).unwrap()
    } else {
        linux
    };
    let child = builds.pop().unwrap();
    let inputs = child.package_inputs().clone();
    let child = child
        .complete(session.compiler(), product_target, inputs.clone())
        .unwrap();
    assert_eq!(child.generated_source_bundle().target(), product_target);
    let host_child = child.select_target(profile, inputs).unwrap();
    let root = builds.pop().unwrap();
    let inputs = root
        .package_inputs()
        .clone()
        .with_complete_dependency_generated_sources(vec![child.generated_source_bundle().clone()])
        .unwrap();
    let root = root
        .complete(session.compiler(), product_target, inputs)
        .unwrap();
    let host_inputs = root
        .package_inputs()
        .clone()
        .with_complete_dependency_generated_sources(vec![
            host_child.generated_source_bundle().clone(),
        ])
        .unwrap();
    let root = root.select_target(profile, host_inputs).unwrap();
    let result = execute_joined_product(session.compiler(), root, &temporary.path().join("output"));
    if private_call {
        let diagnostics = result
            .expect_err("application join must preserve privacy")
            .into_diagnostics();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("selects private machine `private_answer::entry`")),
            "{diagnostics:?}"
        );
        println!("application_executed_source_join_preserved_private_machine_rejection");
    } else {
        result
            .unwrap_or_else(|error| panic!("application product: {:?}", error.into_diagnostics()));
        println!(
            "{project}: application_executed_source_join_native_and_terminal_exit0_without_live_sources_or_replay"
        );
    }
    assert_eq!(counters::build_execution_count(), before + 2);
    assert_eq!(
        sponsor.consumed_fuel_units(),
        discovery_fuel,
        "Product cannot replay Build"
    );
    assert_eq!(
        resolved.prepared.entry_path(),
        temporary
            .path()
            .canonicalize()
            .unwrap()
            .join(project)
            .join("main.omg")
    );
    let admitted = package_manager::operations::admit_project_compilation(
        package_manager::operations::ProjectCompilationAdmissionRequest {
            prepared: resolved.prepared,
            target_profiles: &[profile],
            snapshot: None,
            verify: true,
            timings: false,
            product: compiler::CompilationProduct::Check,
        },
        &mut session,
    );
    assert_eq!(
        admitted.requests.len(),
        1,
        "admit the executed source graph after live deletion"
    );
    for request in &admitted.requests {
        for occurrence in &request.occurrences {
            let account = occurrence
                .evaluation_sponsor
                .as_ref()
                .unwrap()
                .begin_invocation(sponsor.measurement(), 1)
                .unwrap();
            assert!(
                sponsor.owns_usage(&account.snapshot().unwrap()),
                "later admission must retain the discovery sponsor"
            );
        }
    }
    assert_eq!(counters::build_execution_count(), before + 2);
    drop(admitted);
    session.close().expect("dispose shared staging");
}

fn executed_dependency_source_join(project: &str, private_call: bool) {
    use package_manager::resolution::source::ExecutedSourceResolution;
    let temporary = executed_source_workspace(project, private_call);
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let preparation = package_manager::operations::begin_local_project_preparation(
        &temporary.path().join(project).join("main.omg"),
        &[profile],
        true,
    )
    .expect("recover before source acquisition")
    .expect("authored Build starts project preparation");
    let acquired = preparation
        .acquire_root()
        .expect("capture root before declaration");
    let mut compiler = CompilationSession::default();
    let target = profile.target_name();
    let request = |capture: &package_compilation::CapturedLocalSource| CheckedCompileRequest {
        source_capture: Some(capture.clone()),
        ..CheckedCompileRequest::new(
            &capture.canonical_live_root().join("main.omg"),
            Some(target),
        )
    };
    let before = counters::build_execution_count();
    let mut root_invocation = compiler
        .begin_package_build(request(acquired.source_capture()))
        .unwrap();
    let mut root_return = return_build(&mut root_invocation).unwrap();
    let mut resolution = acquired
        .declare_build(&root_return)
        .unwrap()
        .resolve()
        .unwrap();
    let mut children = Vec::new();
    let mut acquisitions = preparation.source_acquisitions().unwrap();
    let resolved = loop {
        match resolution {
            ExecutedSourceResolution::Complete(resolved) => break resolved,
            ExecutedSourceResolution::Request(pending) => {
                let acquired = match acquisitions
                    .acquire(pending)
                    .expect("acquire only the selected dependency")
                {
                    package_manager::resolution::source::BuildSourceAcquisition::Execute(
                        acquired,
                    ) => acquired,
                    package_manager::resolution::source::BuildSourceAcquisition::Reused(next) => {
                        resolution = next;
                        continue;
                    }
                };
                let capture = acquired.source_capture().clone();
                for name in ["main.omg", "build.omg"] {
                    std::fs::remove_file(capture.canonical_live_root().join(name)).unwrap();
                }
                let mut invocation = compiler.begin_package_build(request(&capture)).unwrap();
                let returned = return_build(&mut invocation).unwrap();
                let (identity, next) = acquired
                    .complete(&returned)
                    .expect("join actual child declaration");
                resolution = next;
                children.push((identity, invocation, returned));
            }
        }
    };
    assert_eq!(
        children.len(),
        1,
        "unselected request must not acquire work"
    );
    for name in ["main.omg", "build.omg"] {
        std::fs::remove_file(acquired.source_capture().canonical_live_root().join(name)).unwrap();
    }
    let preview = resolved
        .compilation_inputs(resolved.root())
        .expect("all packages retain captured bytes");
    assert_eq!(preview.packages().count(), 2);
    let mut generated = Vec::new();
    for (identity, mut invocation, mut returned) in children {
        assert!(
            preview.canonical_source_metadata(identity).is_none(),
            "foreign package has no root Build metadata view"
        );
        let inputs = resolved.join_build(identity, &mut returned).unwrap();
        assert!(
            inputs.canonical_source_metadata(identity).is_some(),
            "selected root gains its own captured Build view"
        );
        invocation.join_product_context(profile, inputs).unwrap();
        let build = compiler
            .complete_package_build(invocation, Some(returned))
            .unwrap_or_else(|error| panic!("child completion: {:?}", error.into_diagnostics()));
        generated.push(build.generated_source_bundle().clone());
    }
    assert!(
        root_return.join_dependency_sources(&preview, &[]).is_err(),
        "missing selection must refuse"
    );
    let selected = preview
        .direct_dependencies(build_declarations::DependencyPurpose::Product)
        .next()
        .unwrap()
        .1;
    for (purpose, request_ordinal, package, reason) in [
        (
            build_declarations::DependencyPurpose::Product,
            1,
            selected,
            "request order",
        ),
        (
            build_declarations::DependencyPurpose::Build,
            0,
            selected,
            "no acquired selection",
        ),
        (
            build_declarations::DependencyPurpose::Product,
            0,
            preview.root(),
            "alias and purpose",
        ),
    ] {
        let failure = root_return
            .join_dependency_sources(
                &preview,
                &[build_evaluation::BuildDependencySelection {
                    purpose,
                    request_ordinal,
                    package,
                }],
            )
            .expect_err("incorrect source selection must refuse");
        assert!(failure.contains(reason), "{failure}");
    }
    let inputs = resolved
        .join_build(resolved.root(), &mut root_return)
        .unwrap()
        .with_complete_dependency_generated_sources(generated)
        .unwrap();
    let changed_graph = unbound_join_inputs(
        acquired.source_capture(),
        acquired.source_capture().tree().clone(),
        project,
    );
    assert!(
        root_return
            .validate_source_join(Some(&changed_graph), true)
            .expect_err("completion cannot substitute a different graph")
            .contains("differs from its executed request join")
    );
    assert!(
        resolved
            .join_build(resolved.root(), &mut root_return)
            .is_err(),
        "request join is single-use"
    );
    drop(acquisitions);
    let prepared = preparation
        .complete_source_resolution(resolved)
        .expect("complete preparation from actual Build source work");
    assert_eq!(
        prepared.entry_path(),
        acquired
            .source_capture()
            .canonical_live_root()
            .join("main.omg")
    );
    root_invocation
        .join_product_context(profile, inputs)
        .unwrap();
    let build = compiler
        .complete_package_build(root_invocation, Some(root_return))
        .unwrap_or_else(|error| panic!("joined root completion: {:?}", error.into_diagnostics()));
    let outcome = execute_joined_product(&mut compiler, build, &temporary.path().join("output"));
    if private_call {
        let diagnostics = outcome
            .expect_err("package join cannot expose private machines")
            .into_diagnostics();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("selects private machine `private_answer::entry`")),
            "{diagnostics:?}"
        );
        println!("executed_dependency_join_preserved_private_machine_rejection");
    } else {
        outcome
            .unwrap_or_else(|failure| panic!("joined Product: {:?}", failure.into_diagnostics()));
        println!(
            "{project}: executed_dependency_graph_joined_captured_child_native_and_terminal_exit0_without_replay"
        );
    }
    assert_eq!(
        counters::build_execution_count(),
        before + 2,
        "one root and one dependency activation, no replay"
    );
}

fn project_session_lifecycle() {
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("project-session", false)
            .unwrap();
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/project_session_shared_budget_exit");
    let project = temporary.path().join("project");
    std::fs::create_dir(&project).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let root = project.join("main.omg");
    let before = counters::build_execution_count();
    let mut outcomes = omega_build::build(omega_build::BuildRequest {
        root_path: root.clone(),
        build_directory: Some(temporary.path().join("ordinary")),
        targets: omega_build::BuildTargets::Single(profile),
        product: omega_build::ProjectProduct::Native {
            destination: omega_build::NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    let output = outcomes
        .pop()
        .unwrap()
        .result
        .unwrap()
        .output
        .into_report()
        .unwrap();
    execute_report(output, &temporary.path().join("native"));
    assert_eq!(counters::build_execution_count(), before + 1);

    let preparation = || {
        package_manager::operations::begin_local_project_preparation(&root, &[profile], true)
            .unwrap()
            .unwrap()
    };
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("limited")).unwrap();
    let sponsor = session.evaluation_sponsor().clone();
    // Model earlier work spending the invocation's entire result allowance.
    // Source discovery must use this same sponsor and refuse its own result.
    let maximum = sponsor.limits().maximum_result_storage_cells();
    sponsor
        .begin_invocation(sponsor.measurement(), 1)
        .unwrap()
        .charge_result_custody(maximum, 0)
        .unwrap();
    let before = counters::build_execution_count();
    let error = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation: preparation(),
            execution_profile: profile,
            snapshot: None,
        },
    )
    .err()
    .expect("source work cannot replenish a spent allowance");
    match error {
        omega_build::ProjectSourceResolutionError::Compilation(failure) => {
            let diagnostics = failure.into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("evaluation ResultCustody allowance exhausted")),
                "{diagnostics:?}"
            );
        }
        error => panic!("expected shared result budget refusal: {error:?}"),
    }
    assert_eq!(sponsor.consumed_result_storage_cells(), maximum);
    assert_eq!(counters::build_execution_count(), before + 1);
    session.close().unwrap();
    let before = counters::build_execution_count();
    let error = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation: preparation(),
            execution_profile: profile,
            snapshot: None,
        },
    )
    .err()
    .expect("closed session cannot admit another source Build");
    assert!(
        matches!(
            error,
            omega_build::ProjectSourceResolutionError::Resources(_)
        ),
        "{error:?}"
    );
    assert_eq!(counters::build_execution_count(), before);
    println!(
        "ordinary_project_native_and_terminal_exit0_shared_discovery_budget_and_closed_session_refusals"
    );
}

fn completed_build_target_fanout() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-target-fanout",
        false,
    )
    .unwrap();
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/build/project_session_shared_budget_exit");
    let project = temporary.path().join("project");
    std::fs::create_dir(&project).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture.join(name), project.join(name)).unwrap();
    }
    let host = target::TargetProfile::host_if_supported().unwrap();
    let preparation = package_manager::operations::begin_local_project_preparation(
        &project.join("main.omg"),
        &[host],
        true,
    )
    .unwrap()
    .unwrap();
    let mut session =
        package_manager::PackageBuildSession::create(&temporary.path().join("session")).unwrap();
    let before = counters::build_execution_count();
    let mut resolved = omega_build::resolve_project_sources(
        &mut session,
        omega_build::ProjectSourceRequest {
            preparation,
            execution_profile: host,
            snapshot: None,
        },
    )
    .unwrap();
    let build = resolved.builds.pop().unwrap();
    let inputs = build.package_inputs().clone();
    let build = build
        .complete(session.compiler(), host, inputs.clone())
        .unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::remove_file(project.join(name)).unwrap();
    }
    let windows = target::TargetProfile::from_omega_target_name(Some("windows_x86_64")).unwrap();
    let linux = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let unbound_target =
        target::TargetProfile::from_omega_target_name(Some("uefi_x86_64")).unwrap();
    let mut products = Vec::new();
    for profile in [windows, linux, unbound_target] {
        let selected = build.select_target(profile, inputs.clone()).unwrap();
        assert_eq!(selected.generated_source_bundle().target(), profile);
        products.push((
            (profile, false),
            compiler::PreparedPackageProduct {
                build: selected,
                product: compiler::CompilationProduct::Terminal(Default::default()),
                permissions: None,
            },
        ));
    }
    let wrong_purpose = inputs
        .clone()
        .with_compilation_purpose(build_declarations::DependencyPurpose::Build)
        .unwrap();
    let failure = build
        .select_target(host, wrong_purpose)
        .err()
        .expect("target selection cannot reinterpret Product as Build scope");
    assert!(failure.into_diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot change its source or semantic admission")
    }));
    products.push(((host, true), compiler::PreparedPackageProduct {
        build,
        product: compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
            private_entry_ram: Vec::new(),
            terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
            terminal_authority_permission_policy: None,
            optimization_rollback: Default::default(),
        }),
        permissions: None,
    }));
    let completions = counters::product_source_completion_count();
    let outcomes = session
        .compiler()
        .complete_package_products(products)
        .unwrap();
    assert_eq!(outcomes.len(), 4);
    for ((profile, native), outcome) in outcomes {
        if profile == unbound_target {
            let diagnostics = outcome
                .err()
                .expect("missing UEFI entry must refuse only this target")
                .into_diagnostics();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                    "selected target `uefi_x86_64` has no bound required root slot `uefi_x86_64::ProgramEntry`"
                    )),
                "{diagnostics:?}"
            );
            continue;
        }
        let report = outcome.unwrap().into_report().unwrap();
        if native {
            execute_report(report, &temporary.path().join("native"));
        } else {
            let artifact = report.artifact().expect("portable target artifact");
            let decoded =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap();
            assert_eq!(artifact.view().identity(), decoded.view().identity());
        }
    }
    assert_eq!(counters::product_source_completion_count(), completions + 1);
    assert_eq!(counters::build_execution_count(), before + 1);
    session.close().unwrap();
    println!(
        "one_returned_build_one_product_source_two_portable_targets_host_native_terminal_exit0_and_isolated_target_refusal"
    );
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
    let refused = first
        .join_product_context(
            target::TargetProfile::from_omega_target_name(Some("windows_x86_64")).unwrap(),
            request().package_inputs.unwrap(),
        )
        .err()
        .expect("a pre-admitted occurrence cannot replace its Product target");
    assert!(refused.into_diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot replace its admitted Product target")
    }));
    assert_eq!(counters::build_execution_count(), before);
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
