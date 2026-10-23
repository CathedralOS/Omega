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
    let measured = BuildEvaluationSponsor::new(limits(1_000_000));
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
    let sponsor = BuildEvaluationSponsor::new(limits(2 * fuel));
    let before = counters::build_execution_count();
    check(&mut session, &sponsor, "windows_x86_64")
        .result
        .expect("first row");
    check(&mut session, &sponsor, "linux_x86_64")
        .result
        .expect("second row");
    let executions = counters::build_execution_count() - before;
    println!(
        "two_rows_executions={executions} charged_fuel={} charged_cells={}",
        sponsor.consumed_fuel_units(),
        sponsor.consumed_result_storage_cells()
    );
    assert_eq!(executions, 1);
    assert_eq!(
        sponsor.consumed_fuel_units(),
        2 * fuel,
        "warm occurrence owes its logical fuel"
    );
    assert_eq!(sponsor.consumed_result_storage_cells(), 2 * cells);
    let fresh = BuildEvaluationSponsor::new(limits(2 * fuel));
    check(&mut session, &fresh, "linux_x86_64")
        .result
        .expect("fresh account");
    assert_eq!(fresh.consumed_fuel_units(), fuel);
    assert_eq!(fresh.consumed_result_storage_cells(), cells);
    assert_eq!(counters::build_execution_count() - before, 1);
    println!("fresh_account_charged_without_execution");

    let mut limited_session = CompilationSession::default();
    let limited = BuildEvaluationSponsor::new(limits(2 * fuel - 1));
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
    let cold = BuildEvaluationSponsor::new(limits(2 * fuel - 1));
    check(&mut CompilationSession::default(), &cold, "windows_x86_64")
        .result
        .expect("affordable cold row");
    let before_cold = counters::build_execution_count();
    let failure = check(&mut CompilationSession::default(), &cold, "linux_x86_64")
        .result
        .expect_err("cold occurrence must reject the same allowance");
    assert!(failure.into_diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("aggregate fuel sponsor exhausted")
    }));
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
    let result_limited = BuildEvaluationSponsor::new(result_limits);
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
}
