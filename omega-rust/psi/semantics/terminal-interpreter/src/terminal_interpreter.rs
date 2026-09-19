//! Fuel-bounded reference execution for verified terminal-Psi artifacts.
//!
//! The public entry accepts only canonical semantic/proof bytes and an
//! admission profile. It decodes and verifies those bytes before constructing
//! execution state; no source or checked-tree representation crosses this
//! boundary.
//!
//! `entry.rs` holds the public entries, `execution.rs` the engine they drive,
//! `values.rs`, `effects.rs`, `results.rs` and `errors.rs` what crosses its
//! boundary; the remaining modules each own one runtime concern.

mod block_bindings;
mod boolean_float_operations;
mod boundary_byte_buffers;
mod byte_sequence_binding;
mod byte_sequence_subslice;
mod byte_sequence_view;
mod byte_sequence_write;
mod call_frames;
mod call_operations;
mod case_membership;
mod custody;
mod effect_results;
mod effects;
mod entry;
mod errors;
mod execution;
mod integer_operations;
#[cfg(test)]
mod placed_view_input_tests;
mod placed_views;
mod primitive_storage;
mod record;
mod reference;
mod results;
mod scalar_array;
mod scalar_case_arguments;
mod scalar_operations;
#[cfg(test)]
mod scalar_provider_call_tests;
mod semantic_value_comparison;
#[cfg(test)]
mod structural_argument_binding_tests;
mod structural_byte_arrays;
mod structural_byte_sequence_index_store;
mod structural_byte_sequence_store;
mod structural_operations;
mod structural_scalar_fields;
mod terminators;
mod values;

pub use boundary_byte_buffers::TerminalBoundaryByteBuffer;
pub use case_membership::TerminalStructuralCaseValue;
pub use effect_results::TerminalEffectResult;
pub use effects::{
    AcceptTerminalEffects, AdmittedProviderInstallation, ProviderInstallationSelection,
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
};
pub use entry::{
    TerminalStructuralInputs, admit_provider_installation_from_artifact,
    interpret_serialized_terminal_artifact_measured, interpret_terminal_artifact,
    interpret_terminal_artifact_measured,
};
pub use errors::{
    ProviderInstallationError, TerminalArtifactInterpretError, TerminalInterpretError,
};
pub use execution::TerminalExecution;
pub use placed_views::TerminalPlacedViewEstablishment;
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
pub use structural_byte_arrays::TerminalStructuralByteArrayValue;
pub use structural_scalar_fields::TerminalStructuralScalarFieldValue;
pub use values::{
    TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
