//! Fuel-bounded reference execution of admitted Terminal Psi artifacts.
//!
//! Start at `terminal_interpreter.rs`: it owns the entries and the sequence
//! every one of them runs — decode the canonical semantic and proof bytes,
//! verify them under the admission profile, construct the execution, run it.
//! `execution.rs` is the engine those entries drive: the live state and the
//! interpreter loop. Execution retains immutable code and resumable live
//! custody; fuel exhaustion is not a semantic program outcome.
//!
//! The domains beside them, by responsibility: `structural_inputs` (the
//! host-supplied entry inputs and their binding), `custody` (claims, the
//! affine frontier and argument binding), `reference`, `primitive_storage`,
//! `record`, `scalar_array` and `byte_sequences` (the storage the loop
//! mutates), `scalar_operations`, `structural_operations`, `calls` and
//! `terminators` (what the loop dispatches), `effects` (what it hands to the
//! host and how host results bind), `values`, `results` and `errors` (what
//! crosses the boundary) and `semantic_value_comparison` (trace-value
//! comparison for differential checks).

mod byte_sequences;
mod calls;
mod custody;
mod effects;
mod element_views;
mod errors;
mod execution;
mod primitive_storage;
mod record;
mod reference;
mod results;
mod runtime_indexes;
mod scalar_array;
mod scalar_operations;
mod semantic_value_comparison;
mod structural_inputs;
mod structural_operations;
mod terminal_interpreter;
mod terminators;
mod values;

pub use effects::byte_buffers::TerminalBoundaryByteBuffer;
pub use effects::results::TerminalEffectResult;
pub use effects::{
    AcceptTerminalEffects, AdmittedProviderInstallation, ProviderInstallationSelection,
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
};
pub use errors::{
    ProviderInstallationError, TerminalArtifactInterpretError, TerminalInterpretError,
};
pub use execution::TerminalExecution;
pub use results::{
    MeasuredTerminalExecution, TerminalCrash, TerminalCrashSite, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalScalarCaseResult, TerminalStructuralResult,
};
pub use scalar_array::{TerminalScalarArrayResult, TerminalScalarArrayValue};
pub use semantic_value_comparison::{
    TerminalTraceScalarComparisonError, TerminalTraceScalarValueSide,
    TerminalTraceStructuralComparisonError, TerminalTraceStructuralValueSide,
    compare_terminal_trace_scalar_values, compare_terminal_trace_structural_values,
};
pub use structural_inputs::TerminalStructuralInputs;
pub use structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
pub use structural_inputs::case_membership::TerminalStructuralCaseValue;
pub use structural_inputs::placed_views::TerminalPlacedViewEstablishment;
pub use structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
pub use terminal_interpreter::{
    admit_provider_installation_from_artifact, interpret_serialized_terminal_artifact_measured,
    interpret_terminal_artifact, interpret_terminal_artifact_measured,
};
pub use values::{
    TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
