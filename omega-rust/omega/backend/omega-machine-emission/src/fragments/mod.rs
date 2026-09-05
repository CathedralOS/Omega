//! Optimizer module role: executable entrance. Resolved machine data to unplaced fragments.
//!
//! These functions construct and check the fragment projection only. The caller
//! must independently admit the selected program, layout, and realization before
//! publication. Neither raw input nor a successful projection grants that authority.

mod production;
mod validation;

use omega_machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineProgram};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFragmentEmissionError {
    MissingFunction(psi_core::MachineId),
    MissingBlock(omega_selected_instructions::SelectedBlockId),
    MissingInstruction(omega_selected_instructions::SelectedInstructionId),
    OffsetOverflow,
    RootMismatch,
    ArtifactMismatch,
}

impl std::fmt::Display for ResolvedFragmentEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "resolved fragment projection failed: {self:?}")
    }
}

impl std::error::Error for ResolvedFragmentEmissionError {}

pub fn emit_resolved_function_fragments(
    program: &ResolvedMachineProgram,
) -> Result<FunctionFragmentEmissionPlan, ResolvedFragmentEmissionError> {
    let fragments = production::emit(program)?;
    validate_resolved_function_fragments(program, &fragments)?;
    Ok(fragments)
}

/// Check claimed bytes, spans, provenance, and unresolved fixups without emitting
/// a second fragment program or consulting optimization history.
pub fn validate_resolved_function_fragments(
    program: &ResolvedMachineProgram,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    validation::check(program, fragments)
}
