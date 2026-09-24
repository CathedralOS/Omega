//! Projection of resolved machine data into unplaced function fragments.
//!
//! [`emit_resolved_function_fragments`] builds each function's fragment bytes,
//! spans, provenance and unresolved fixups from the resolved layout
//! (`production`) and returns only after the independent checker
//! (`validation`) accepts them; `statistics` counts the result. The
//! fragment-emission stage above admits the selected program, layout and
//! realization; neither raw input nor a successful projection grants that
//! authority.

mod production;
mod statistics;
mod validation;

pub use statistics::{FunctionFragmentStatisticsOverflow, function_fragment_emission_statistics};

use machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineProgram};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFragmentEmissionError {
    MissingFunction(semantic_vocabulary::MachineId),
    MissingBlock(selected_instructions::SelectedBlockId),
    MissingInstruction(selected_instructions::SelectedInstructionId),
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
