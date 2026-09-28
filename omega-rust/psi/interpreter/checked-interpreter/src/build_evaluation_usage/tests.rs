use super::{BuildEvaluationUsage, BuildEvaluationUsageError};
use crate::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationUsage};

fn measured_usage() -> EvaluationUsage {
    EvaluationUsage {
        fuel_units: 2,
        build_log_bytes: 3,
        filesystem_operation_attempts: 4,
        peak_live_cells: 1,
        peak_live_text_bytes: 5,
        result_cells: 6,
        result_text_bytes: 7,
        ..EvaluationUsage::empty(100)
    }
}

fn sponsored_usage() -> BuildEvaluationUsage {
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(101, 102, 103, 104, 105, 106, 107, 108).unwrap(),
    );
    let handle = sponsor.reserve_live_filesystem_handle().unwrap();
    let cell = sponsor.reserve_live_cell().unwrap();
    let text = sponsor.reserve_live_text_bytes(5).unwrap();
    drop((handle, cell, text));
    BuildEvaluationUsage::capture(measured_usage(), Some(&sponsor))
}

#[test]
fn capture_retains_invocation_fields_and_sponsor_peaks_after_release() {
    let usage = measured_usage();
    let sponsored = sponsored_usage();
    assert_eq!(
        sponsored,
        BuildEvaluationUsage {
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            invocation_fuel_ceiling: 100,
            sponsor_schema_version: Some(crate::BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION),
            session_fuel_ceiling: Some(101),
            session_build_log_byte_ceiling: Some(102),
            session_filesystem_attempt_ceiling: Some(103),
            session_live_filesystem_handle_ceiling: Some(104),
            session_live_cell_ceiling: Some(105),
            session_live_text_byte_ceiling: Some(106),
            session_result_cell_ceiling: Some(107),
            session_result_text_byte_ceiling: Some(108),
            session_peak_live_filesystem_handles: 1,
            session_peak_live_cells: 1,
            session_peak_live_text_bytes: 5,
            fuel_units: 2,
            build_log_bytes: 3,
            filesystem_operation_attempts: 4,
            peak_live_cells: 1,
            peak_live_text_bytes: 5,
            result_cells: 6,
            result_text_bytes: 7,
        }
    );
    sponsored.validate().expect("valid sponsored snapshot");
    let standalone = BuildEvaluationUsage::capture(usage, None);
    standalone.validate().expect("valid unsponsored invocation");
    assert_eq!(standalone.sponsor_schema_version, None);
    assert_eq!(standalone.session_peak_live_cells, 0);
    assert_eq!(standalone.session_peak_live_text_bytes, 0);
    assert!(sponsored.has_same_invocation_usage(standalone));
}

#[test]
fn retained_usage_checks_each_ceiling_and_invocation_session_relationship() {
    use BuildEvaluationUsageError::*;
    type Mutation = fn(&mut BuildEvaluationUsage);
    let cases: &[(Mutation, BuildEvaluationUsageError)] = &[
        (
            |usage| usage.invocation_fuel_ceiling = 0,
            ZeroInvocationFuelCeiling,
        ),
        (|usage| usage.fuel_units = 101, InvocationFuelExceeded),
        (
            |usage| usage.session_fuel_ceiling = Some(0),
            ZeroSessionFuelCeiling,
        ),
        (
            |usage| usage.session_build_log_byte_ceiling = Some(0),
            ZeroSessionBuildLogCeiling,
        ),
        (
            |usage| usage.session_filesystem_attempt_ceiling = Some(0),
            ZeroSessionFilesystemAttemptCeiling,
        ),
        (
            |usage| usage.session_live_filesystem_handle_ceiling = Some(0),
            ZeroLiveFilesystemHandleCeiling,
        ),
        (
            |usage| usage.session_live_cell_ceiling = Some(0),
            ZeroLiveCellCeiling,
        ),
        (
            |usage| usage.session_live_text_byte_ceiling = Some(0),
            ZeroLiveTextByteCeiling,
        ),
        (
            |usage| usage.session_result_cell_ceiling = Some(0),
            ZeroResultCellCeiling,
        ),
        (
            |usage| usage.session_result_text_byte_ceiling = Some(0),
            ZeroResultTextByteCeiling,
        ),
        (
            |usage| usage.session_fuel_ceiling = Some(1),
            SessionFuelExceeded,
        ),
        (|usage| usage.build_log_bytes = 103, SessionBuildLogExceeded),
        (
            |usage| usage.filesystem_operation_attempts = 104,
            SessionFilesystemAttemptsExceeded,
        ),
        (
            |usage| usage.session_peak_live_filesystem_handles = 105,
            LiveFilesystemHandlesExceeded,
        ),
        (
            |usage| usage.session_peak_live_cells = 106,
            LiveCellsExceeded,
        ),
        (|usage| usage.peak_live_cells = 106, LiveCellsExceeded),
        (
            |usage| usage.peak_live_cells = 2,
            InvocationCellPeakExceedsSession,
        ),
        (
            |usage| usage.session_peak_live_text_bytes = 107,
            LiveTextBytesExceeded,
        ),
        (
            |usage| usage.peak_live_text_bytes = 107,
            LiveTextBytesExceeded,
        ),
        (
            |usage| usage.peak_live_text_bytes = 6,
            InvocationTextPeakExceedsSession,
        ),
        (|usage| usage.result_cells = 108, ResultCellsExceeded),
        (
            |usage| usage.result_text_bytes = 109,
            ResultTextBytesExceeded,
        ),
        (
            |usage| usage.session_fuel_ceiling = None,
            IncompleteSponsorIdentity,
        ),
        (
            |usage| usage.sponsor_schema_version = None,
            IncompleteSponsorIdentity,
        ),
    ];
    let original = sponsored_usage();
    for (mutate, expected) in cases {
        let mut changed = original;
        mutate(&mut changed);
        assert_eq!(changed.validate(), Err(*expected), "{expected:?}");
    }
    let standalone = BuildEvaluationUsage::capture(measured_usage(), None);
    for mutate in [
        (|usage: &mut BuildEvaluationUsage| usage.session_peak_live_cells = 1) as Mutation,
        |usage| usage.session_peak_live_text_bytes = 1,
        |usage| usage.session_peak_live_filesystem_handles = 1,
    ] {
        let mut changed = standalone;
        mutate(&mut changed);
        assert_eq!(changed.validate(), Err(UnsponsoredSessionUsage));
    }
}

#[test]
fn exact_ceilings_admit_and_multiple_failures_keep_invocation_first() {
    let mut usage = sponsored_usage();
    usage.invocation_fuel_ceiling = usage.fuel_units;
    usage.session_fuel_ceiling = Some(usage.fuel_units);
    usage.session_build_log_byte_ceiling = Some(usage.build_log_bytes);
    usage.session_filesystem_attempt_ceiling = Some(usage.filesystem_operation_attempts);
    usage.session_live_filesystem_handle_ceiling = Some(usage.session_peak_live_filesystem_handles);
    usage.session_live_cell_ceiling = Some(usage.peak_live_cells);
    usage.session_live_text_byte_ceiling = Some(usage.peak_live_text_bytes);
    usage.session_result_cell_ceiling = Some(usage.result_cells);
    usage.session_result_text_byte_ceiling = Some(usage.result_text_bytes);
    usage
        .validate()
        .expect("equality is within the declared ceilings");
    usage.invocation_fuel_ceiling = 0;
    usage.session_fuel_ceiling = Some(0);
    assert_eq!(
        usage.validate(),
        Err(BuildEvaluationUsageError::ZeroInvocationFuelCeiling)
    );
}

#[test]
fn invocation_comparison_ignores_only_sponsorship() {
    type Mutation = fn(&mut BuildEvaluationUsage);
    let original = sponsored_usage();
    let mutations: &[Mutation] = &[
        |usage| usage.usage_schema_version += 1,
        |usage| usage.step_schedule_marker += 1,
        |usage| usage.invocation_fuel_ceiling += 1,
        |usage| usage.fuel_units += 1,
        |usage| usage.build_log_bytes += 1,
        |usage| usage.filesystem_operation_attempts += 1,
        |usage| usage.peak_live_cells += 1,
        |usage| usage.peak_live_text_bytes += 1,
        |usage| usage.result_cells += 1,
        |usage| usage.result_text_bytes += 1,
    ];
    for mutate in mutations {
        let mut changed = original;
        mutate(&mut changed);
        assert!(!original.has_same_invocation_usage(changed));
    }
    let mut changed = original;
    changed.sponsor_schema_version = Some(99);
    changed.session_fuel_ceiling = Some(1000);
    changed.session_peak_live_cells = 100;
    assert!(original.has_same_invocation_usage(changed));
}
