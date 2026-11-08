//! Transitional checked-tree projection into independently owned accounting data.
//! The current evaluator supplies real private usage; this projection cannot
//! reinterpret Terminal fuel or manufacture live-resource measurements.

use crate::{BuildEvaluationSponsor, EvaluationUsage};
use evaluation_accounting::{
    BuildEvaluationUsage, BuildEvaluationUsageError, EvaluationMeasurementIdentity,
};

/// Capture the measured invocation and the sponsor's current peaks at the
/// caller's publication point. Session context is not invocation identity.
pub fn capture_checked_build_evaluation_usage(
    usage: EvaluationUsage,
    sponsor: Option<&BuildEvaluationSponsor>,
) -> Result<BuildEvaluationUsage, BuildEvaluationUsageError> {
    if sponsor
        .is_some_and(|sponsor| sponsor.measurement() != EvaluationMeasurementIdentity::CheckedTree)
    {
        return Err(BuildEvaluationUsageError::MeasurementSponsorMismatch);
    }
    let record = BuildEvaluationUsage {
        measurement: evaluation_accounting::EvaluationMeasurementIdentity::CheckedTree,
        usage_schema_version: usage.schema().schema_version(),
        step_schedule_marker: usage.schedule().marker(),
        invocation_fuel_ceiling: usage.fuel_ceiling(),
        sponsor_schema_version: sponsor.map(|sponsor| sponsor.limits().schema_version()),
        session_fuel_ceiling: sponsor.map(|sponsor| sponsor.limits().maximum_fuel_units()),
        session_build_log_byte_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_build_log_bytes()),
        session_filesystem_attempt_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_filesystem_operation_attempts()),
        session_live_filesystem_handle_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_live_filesystem_handles()),
        session_live_storage_cell_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_live_cells()),
        session_live_payload_byte_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
        session_result_storage_cell_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_result_cells()),
        session_result_payload_byte_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
        session_peak_live_filesystem_handles: sponsor
            .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
        session_peak_live_storage_cells: sponsor.map_or(0, BuildEvaluationSponsor::peak_live_cells),
        session_peak_live_payload_bytes: sponsor
            .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
        fuel_units: usage.fuel_units(),
        build_log_bytes: usage.build_log_bytes(),
        filesystem_operation_attempts: usage.filesystem_operation_attempts(),
        peak_live_storage_cells: usage.peak_live_cells(),
        peak_live_payload_bytes: usage.peak_live_text_bytes(),
        result_storage_cells: usage.result_cells(),
        result_payload_bytes: usage.result_text_bytes(),
    };
    record.validate()?;
    Ok(record)
}
