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
