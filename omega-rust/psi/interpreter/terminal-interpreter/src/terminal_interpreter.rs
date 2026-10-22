//! The raw start entry for Terminal-Psi execution.
//!
//! [`TerminalExecution::start_artifact`] accepts canonical semantic bytes; no
//! source or checked-tree representation crosses this boundary. It decodes the
//! module once, binds the supplied scalar arguments and
//! [`TerminalStructuralInputs`] before committing any operation, and returns
//! the engine for a caller that drives `TerminalExecution::resume` itself,
//! charging fuel and dispatching every operation and terminator until the
//! artifact completes, crashes or the sponsor's fuel is exhausted.
//!
//! Provider admission borrows an already retained artifact through
//! `terminal_installation::admit_provider_installation`, or decodes external
//! semantic bytes through that owner's artifact entry. Shared admission checks
//! policy against the catalog without constructing runtime state.
//!
//! The domains beside this file: `execution` owns the live state and the
//! interpreter loop; `structural_inputs` the host-supplied entry inputs and
//! their binding; `custody`, `reference`, `primitive_storage`, `record` and
//! `byte_sequences` the ownership and storage the loop mutates;
//! `scalar_operations`, `structural_operations`, `calls` and `terminators`
//! the operations the loop dispatches; `effects` what the loop hands to the
//! host; and `values`, `results` and `errors` what crosses the boundary.

use crate::errors::TerminalArtifactInterpretError;
use crate::execution::TerminalExecution;
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;

impl TerminalExecution<'_> {
    /// Decode one artifact, then bind its scalar arguments and structural
    /// inputs before committing any operation or custody transfer.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        // This raw start and `TerminalProgram::instantiate` share the same
        // immutable program and input binding.
        crate::program::TerminalProgram::load(semantic_bytes)?.start_execution(
            scalar_arguments,
            structural_inputs,
            None,
        )
    }
}
