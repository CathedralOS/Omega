//! Runs the admitted machine once and reports its measured result or failure.

use build_time_evaluation::BuildMachineInvocation;
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildTimeValue, PreparedBuildMachineEntry,
    PreparedBuildMachineProgram,
};
use checked_interpreter::MeasuredBuildMachineEvaluation;
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
    let product_entry_compatibility =
        provider_planning::program_entry::ProductEntryQueryCompatibility;
    build_time_evaluation::evaluate_build_machine_measured(
        prepared,
        BuildMachineInvocation {
            entry: machine_entry,
            arguments,
            mode,
            sponsor,
            product_entry_compatibility: Some(&product_entry_compatibility),
        },
    )
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
                let logical_handle_operands = attempts
                    .iter()
                    .map(|attempt| attempt.logical_handle_inputs().len())
                    .sum::<usize>();
                format!(
                    "; partial non-admission filesystem evidence: {} call(s), {halted} evaluator-halted, {grant_refusals} grant refusal(s), {logical_handle_operands} logical-handle operand(s)",
                    attempts.len()
                )
            })
            .unwrap_or_default();
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}{partial_evidence}"
        ))]
    })
}
