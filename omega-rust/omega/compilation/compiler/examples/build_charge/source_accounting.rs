//! Measure real source Build occurrences through the public check entry.

use super::fixture_package_inputs::{self, ReviewedFixtureCompilation};
use super::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits, CompilationSession, sponsor};
use artifacts::compile_timings::counters;
use compiler::CheckedCompileRequest;
use products::CheckedCompileOutcome;

pub(super) fn check(
    prepared: &ReviewedFixtureCompilation,
    session: &mut CompilationSession,
    account: &BuildEvaluationSponsor,
    target: &str,
) -> CheckedCompileOutcome {
    let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(target));
    request.package_inputs = prepared.package_inputs.clone();
    request.build.profile = target::TargetProfile::host_if_supported();
    request.build.snapshot = package_compilation::BuildSnapshotRequest::automatic_package_inventory(
        request.package_inputs.as_ref(),
    );
    request.build.evaluation_sponsor = Some(account.clone());
    orchestration::check_source(session, request)
}

pub(super) fn run() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/fixtures/run/calls/installed_entry_contract_exit/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some("linux_x86_64"),
    )
    .expect("fixture admission");

    // Give every request its own compilation session. Only the explicitly
    // supplied resource account spans the Windows and Linux occurrences.
    let execute = |account: &BuildEvaluationSponsor, target| {
        let before = counters::source_build_execution_count();
        let result = check(
            &prepared,
            &mut CompilationSession::default(),
            account,
            target,
        )
        .result;
        assert_eq!(counters::source_build_execution_count() - before, 1);
        result
    };
    let calibration = sponsor(super::limits(1_000_000));
    let usage = execute(&calibration, "linux_x86_64")
        .expect("calibration")
        .build()
        .evaluation_usage
        .expect("source Build measured its execution");
    let work = usage.work_units;
    let cells = usage.result_storage_cells;
    assert!(work > 1 && cells > 0);
    assert_eq!(calibration.consumed_work_units(), work);
    assert_eq!(calibration.consumed_result_storage_cells(), cells);
    println!("source_build_work={work} result_storage_cells={cells}");

    for (allowed_work, allowed_cells, refusal) in [
        (2 * work, 2 * cells, None),
        (
            2 * work - 1,
            2 * cells,
            Some("evaluation Work budget exhausted"),
        ),
        (
            2 * work,
            2 * cells - 1,
            Some("evaluation ResultCustody budget exhausted"),
        ),
    ] {
        let limits = BuildEvaluationSponsorLimits::new(
            allowed_work,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
            1_000_000,
            allowed_cells,
            1_000_000,
        )
        .expect("two source occurrences have positive ceilings even when one unit short");
        let account = sponsor(limits);
        execute(&account, "windows_x86_64").expect("first source occurrence fits");
        assert_eq!(account.consumed_work_units(), work);
        assert_eq!(account.consumed_result_storage_cells(), cells);

        let second = execute(&account, "linux_x86_64");
        match refusal {
            None => {
                let output = second.expect("both source occurrences fit the exact ceilings");
                let actual = output.build().evaluation_usage.expect("source usage");
                assert_eq!(actual.work_units, work);
                assert_eq!(actual.result_storage_cells, cells);
                assert_eq!(account.consumed_work_units(), allowed_work);
                assert_eq!(account.consumed_result_storage_cells(), allowed_cells);
                println!("two_source_builds_exact_work_and_storage_accepted");
            }
            Some(expected) => {
                let diagnostics = second
                    .expect_err("one-short ceiling must refuse")
                    .into_diagnostics();
                assert!(
                    diagnostics.iter().any(|d| d.message.contains(expected)),
                    "{diagnostics:?}"
                );
                assert!((work..=allowed_work).contains(&account.consumed_work_units()));
                assert!((cells..=allowed_cells).contains(&account.consumed_result_storage_cells()));
                println!("second_source_build_refused={expected}");
            }
        }
        assert_eq!(account.live_storage_cells(), 0);
        assert_eq!(account.live_payload_bytes(), 0);
    }
}
