//! Running the admitted build machine: the measured evaluation, its exact
//! replay under the recognized filesystem replay, and the usage evidence
//! both runs leave behind.

use crate::evidence::observations::BuildEvaluationUsage;
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemMetadataLayout, BuildTimeValue, PreparedBuildMachineEntry,
    PreparedBuildMachineProgram,
};
use build_time_evaluation::{BuildMachineInvocation, PreparedBuildMachine};
use checked_interpreter::{EvaluationUsage, FilesystemReplay, MeasuredBuildMachineEvaluation};
use diagnostics::Diagnostic;

/// One measured run of the admitted build machine over its initial `Build`.
pub(super) type MeasuredBuildMachine = MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>;

/// Run the admitted machine once in its admitted execution mode. A failed run
/// reports the partial filesystem evidence the evaluator retained.
pub(super) fn evaluate_admitted_machine(
    prepared: &PreparedBuildMachineProgram,
    machine_entry: &PreparedBuildMachineEntry,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
    sponsor: Option<&BuildEvaluationSponsor>,
    machine_name: &str,
) -> Result<MeasuredBuildMachine, Vec<Diagnostic>> {
    match sponsor {
        Some(sponsor) => {
            build_time_evaluation::evaluate_build_machine_measured(prepared, BuildMachineInvocation { machine: PreparedBuildMachine::Entry(machine_entry), arguments, mode, sponsor: Some(sponsor) })
        }
        None => build_time_evaluation::evaluate_build_machine_measured(prepared, BuildMachineInvocation { machine: PreparedBuildMachine::Entry(machine_entry), arguments, mode, sponsor: None }),
    }
    .map_err(|reason| {
        let partial_evidence = reason
            .observations()
            .filter(|observations| !observations.filesystem_operation_attempts().is_empty())
            .map(|observations| {
                let attempts = observations.filesystem_operation_attempts();
                let halted = attempts
                    .iter()
                    .filter(|attempt| {
                        matches!(
                            attempt.outcome(),
                            Some(checked_interpreter::FilesystemOperationAttemptOutcome::EvaluationHalted(_))
                        )
                    })
                    .count();
                let grant_refusals = attempts
                    .iter()
                    .map(|attempt| attempt.grant_refusals().len())
                    .sum::<usize>();
                let scalar_operands = attempts
                    .iter()
                    .map(|attempt| attempt.scalar_operands().len())
                    .sum::<usize>();
                let byte_operands = attempts
                    .iter()
                    .map(|attempt| attempt.byte_operands().len())
                    .sum::<usize>();
                let path_like_operands = attempts
                    .iter()
                    .map(|attempt| attempt.path_like_operands().len())
                    .sum::<usize>();
                let logical_handle_operands = attempts
                    .iter()
                    .map(|attempt| attempt.logical_handle_inputs().len())
                    .sum::<usize>();
                let mutable_carrier_operands = attempts
                    .iter()
                    .map(|attempt| {
                        attempt.mutable_byte_operand_resolutions().len()
                            + attempt.mutable_i64_operand_resolutions().len()
                    })
                    .sum::<usize>();
                let rooted_path_operands = attempts
                    .iter()
                    .map(|attempt| attempt.rooted_path_operand_resolutions().len())
                    .sum::<usize>();
                format!(
                    "; partial non-admission filesystem evidence: {} call(s), {halted} evaluator-halted, {grant_refusals} grant refusal(s), {scalar_operands} scalar operand(s), {byte_operands} immutable byte operand(s), {path_like_operands} path-like operand(s), {rooted_path_operands} rooted-path operand(s), {logical_handle_operands} logical-handle operand(s), {mutable_carrier_operands} mutable-carrier operand(s)",
                    attempts.len()
                )
            })
            .unwrap_or_default();
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}{partial_evidence}"
        ))]
    })
}

/// Run the admitted machine again against the recognized filesystem replay and
/// require the same value, operation record, and executed root bindings.
pub(super) fn replay_admitted_machine(
    prepared: &PreparedBuildMachineProgram,
    machine_entry: &PreparedBuildMachineEntry,
    arguments: Vec<BuildTimeValue>,
    replay: FilesystemReplay,
    sponsor: Option<&BuildEvaluationSponsor>,
    machine_name: &str,
    measured: &MeasuredBuildMachine,
) -> Result<EvaluationUsage, Vec<Diagnostic>> {
    let replay_mode = BuildMachineExecutionMode::Granted {
        filesystem: BuildMachineFilesystemAccess::ReplayFilesystem(replay),
        filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout::default(),
    };
    let replayed = match sponsor {
        Some(sponsor) => build_time_evaluation::evaluate_build_machine_measured(
            prepared,
            BuildMachineInvocation {
                machine: PreparedBuildMachine::Entry(machine_entry),
                arguments,
                mode: replay_mode,
                sponsor: Some(sponsor),
            },
        ),
        None => build_time_evaluation::evaluate_build_machine_measured(
            prepared,
            BuildMachineInvocation {
                machine: PreparedBuildMachine::Entry(machine_entry),
                arguments,
                mode: replay_mode,
                sponsor: None,
            },
        ),
    }
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` failed: {reason}"
        ))]
    })?;
    if replayed.value() != measured.value()
        || replayed.observations() != measured.observations()
        || replayed.executed_root_bindings() != measured.executed_root_bindings()
        || replayed.executed_behavior_exclusions() != measured.executed_behavior_exclusions()
    {
        return Err(vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` changed its result or operation record"
        ))]);
    }
    Ok(replayed.usage())
}

/// The durable usage evidence of the measured run, its replay, and the sponsor
/// session that hosted them.
pub(super) fn evaluation_usage(
    usage: EvaluationUsage,
    replay_usage: Option<EvaluationUsage>,
    sponsor: Option<&BuildEvaluationSponsor>,
) -> BuildEvaluationUsage {
    BuildEvaluationUsage {
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
        session_live_cell_ceiling: sponsor.map(|sponsor| sponsor.limits().maximum_live_cells()),
        session_live_text_byte_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
        session_result_cell_ceiling: sponsor.map(|sponsor| sponsor.limits().maximum_result_cells()),
        session_result_text_byte_ceiling: sponsor
            .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
        session_peak_live_filesystem_handles: sponsor
            .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
        session_peak_live_cells: sponsor.map_or(0, BuildEvaluationSponsor::peak_live_cells),
        session_peak_live_text_bytes: sponsor
            .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
        fuel_units: usage.fuel_units(),
        replay_fuel_units: replay_usage.map_or(0, |usage| usage.fuel_units()),
        build_log_bytes: usage.build_log_bytes(),
        replay_build_log_bytes: replay_usage.map_or(0, |usage| usage.build_log_bytes()),
        filesystem_operation_attempts: usage.filesystem_operation_attempts(),
        replay_filesystem_operation_attempts: replay_usage
            .map_or(0, |usage| usage.filesystem_operation_attempts()),
        peak_live_cells: usage.peak_live_cells(),
        replay_peak_live_cells: replay_usage.map_or(0, |usage| usage.peak_live_cells()),
        peak_live_text_bytes: usage.peak_live_text_bytes(),
        replay_peak_live_text_bytes: replay_usage.map_or(0, |usage| usage.peak_live_text_bytes()),
        result_cells: usage.result_cells(),
        replay_result_cells: replay_usage.map_or(0, |usage| usage.result_cells()),
        result_text_bytes: usage.result_text_bytes(),
        replay_result_text_bytes: replay_usage.map_or(0, |usage| usage.result_text_bytes()),
    }
}
