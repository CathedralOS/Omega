//! Count actual Build execution across target requests in one public invocation.
//! The source binds different Windows/Linux entries; reuse must retain both rows.
//! Run alone: these counters are process-wide. Foreign images are not executed.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use artifacts::compile_timings::counters;
use compiler::{CompilationSession, CompileRequest, RequestedCompileProduct};

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
    let mut session = CompilationSession::default();
    let before = counters::build_execution_count();
    let admissions = counters::preliminary_check_totals().0;
    let checks = counters::settled_check_totals().0;
    for name in ["windows_x86_64", "linux_x86_64"] {
        let profile = target::TargetProfile::from_omega_target_name(Some(name)).expect("profile");
        let mut request = CompileRequest::for_targets(
            prepared.root_path.clone(),
            std::env::temp_dir().join(format!("omega-build-session-{}", std::process::id())),
            vec![Some(profile)],
        )
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
        if let Some(inputs) = prepared.package_inputs.clone() {
            request = request.with_package_inputs(inputs);
        }
        let report = omega_build::compile_sources(&mut session, request)
            .expect("request admission")
            .into_single_report()
            .expect("target publication");
        assert!(report.retained_native_artifact().is_some());
    }
    let builds = counters::build_execution_count() - before;
    println!("target_products=2 build_executions={builds}");
    assert_eq!(
        builds, 1,
        "one invocation must reuse exact pure Build inputs"
    );
    assert_eq!(counters::preliminary_check_totals().0 - admissions, 2);
    let checks = counters::settled_check_totals().0 - checks;
    println!("preliminary_admissions=2 settled_checks={checks}");
    assert_eq!(checks, 2, "one Build check and one final Product check");

    // The same source under a different resource policy is a new admission.
    // In particular, an unsponsored result cannot bypass this account's limit.
    let limits = build_time_evaluation::BuildEvaluationSponsorLimits::new(
        1, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
    )
    .expect("nonzero limits");
    let mut limited =
        compiler::CheckedCompileRequest::new(&prepared.root_path, Some("linux_x86_64"));
    limited.package_inputs = prepared.package_inputs.clone();
    limited.build.profile = target::TargetProfile::host_if_supported();
    limited.build.snapshot = compiler::BuildSnapshotRequest::automatic_package_inventory(
        limited.package_inputs.as_ref(),
    );
    limited.build.evaluation_sponsor = Some(
        build_time_evaluation::BuildEvaluationSponsor::new_with_measurement(
            limits,
            evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
                fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
            },
        )
        .expect("current Terminal measurement"),
    );
    let result = omega_build::check_source(&mut session, limited);
    let diagnostics = match result.result {
        Err(failure) => failure.into_diagnostics(),
        Ok(_) => panic!("fresh fuel authority must not be borrowed from a prior result"),
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("Terminal Build failed: evaluation Fuel allowance exhausted")),
        "unexpected rejection: {diagnostics:?}"
    );
    assert_eq!(counters::build_execution_count() - before, 2);
    println!("fresh_sponsor_limit=rejected");

    let other = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &fixture_package_inputs::repo_root()
            .join("tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry/main.omg"),
        Some("linux_x86_64"),
    )
    .expect("distinct source admission");
    let before_other = counters::build_execution_count();
    let mut request = CompileRequest::for_targets(
        other.root_path,
        std::env::temp_dir().join(format!("omega-build-session-other-{}", std::process::id())),
        vec![Some(target::TargetProfile::LinuxX64)],
    )
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(inputs) = other.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let report = omega_build::compile_sources(&mut session, request)
        .expect("distinct request")
        .into_single_report()
        .expect("distinct native product");
    assert!(report.retained_native_artifact().is_some());
    assert_eq!(counters::build_execution_count() - before_other, 1);
    println!("different_source_build_executions=1");
}
