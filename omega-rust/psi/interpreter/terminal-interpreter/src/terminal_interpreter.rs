//! Fuel-bounded reference execution for Terminal-Psi artifacts.
//!
//! The public entries accept canonical semantic bytes; no source or checked-tree
//! representation crosses this boundary. Every entry runs the same sequence:
//!
//! 1. decode — `terminal_codec::decode_module` reconstructs the module;
//! 2. construct — `TerminalProgram::load` (in `program`) retains the module in
//!    shared immutable form and `start_execution` binds the scalar
//!    and [`TerminalStructuralInputs`] before committing any operation;
//! 3. run — [`TerminalExecution::resume`] charges fuel and dispatches every
//!    operation and terminator until the artifact completes, crashes or the
//!    sponsor's fuel is exhausted.
//!
//! [`interpret_serialized_terminal_artifact_measured`] starts from one
//! portable envelope, [`interpret_terminal_artifact_measured`] from its
//! semantic section, and [`interpret_terminal_artifact`] returns
//! only the semantic result under the accepting effect handler. The two
//! `TerminalExecution` starts below are steps 1 and 2 for a caller that drives
//! `resume` itself. Provider admission borrows an already retained artifact
//! through `terminal_installation::admit_provider_installation`, or decodes
//! external semantic bytes through that owner's artifact entry. Shared
//! admission checks policy against the catalog without constructing runtime state.
//!
//! The domains beside this file: `execution` owns the live state and the
//! interpreter loop; `structural_inputs` the host-supplied entry inputs and
//! their binding; `custody`, `reference`, `primitive_storage`, `record` and
//! `byte_sequences` the ownership and storage the loop mutates;
//! `scalar_operations`, `structural_operations`, `calls` and `terminators`
//! the operations the loop dispatches; `effects` what the loop hands to the
//! host; `values`, `results` and `errors` what crosses the boundary; and
//! `semantic_value_comparison` the trace-value comparison used by
//! differential checks.

use crate::effects::{AcceptTerminalEffects, TerminalEffectHandler};
use crate::errors::{TerminalArtifactInterpretError, TerminalInterpretError};
use crate::execution::TerminalExecution;
use crate::results::{MeasuredTerminalExecution, TerminalExecutionResult, TerminalExecutionStatus};
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;
use terminal_fuel::{FuelMeterError, TerminalFuelMeter};

/// Decode one complete portable Terminal-Psi envelope and execute its module
/// with a fresh effect-policy
/// input supplied by the receiver. The envelope contains no checked-tree or
/// build-process object.
pub fn interpret_serialized_terminal_artifact_measured(
    artifact_bytes: &[u8],
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let program = crate::program::TerminalProgram::load_envelope(artifact_bytes)?;
    let execution = program.start_execution(scalar_arguments, structural_inputs, None)?;
    execute_measured(execution, handler)
}

/// Decode and run one artifact to completion under `handler`,
/// measuring its fuel; every structural input arrives in one record.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let execution =
        TerminalExecution::start_artifact(semantic_bytes, scalar_arguments, structural_inputs)?;
    execute_measured(execution, handler)
}

fn execute_measured(
    mut execution: TerminalExecution<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume(&mut meter, handler)
        .map_err(TerminalArtifactInterpretError::Execution)?
    {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Fuel(FuelMeterError::Exhausted(exhaustion)),
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(crash),
            ));
        }
        TerminalExecutionStatus::ExternalCompletion(completion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ExternalCompletion(completion),
            ));
        }
    };
    let structural_primitive_values = execution.final_structural_primitive_values();
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
        effects: execution.effects,
        structural_primitive_values,
    })
}

/// Decode and execute canonical Terminal-Psi semantic bytes, returning only
/// their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(
        semantic_bytes,
        arguments,
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .map(MeasuredTerminalExecution::into_value)
}

impl TerminalExecution<'_> {
    /// Decode one artifact, then bind its scalar arguments and structural
    /// inputs before committing any operation or custody transfer.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        // Both public surfaces use the same immutable program and input binding.
        crate::program::TerminalProgram::load(semantic_bytes)?.start_execution(
            scalar_arguments,
            structural_inputs,
            None,
        )
    }
}
