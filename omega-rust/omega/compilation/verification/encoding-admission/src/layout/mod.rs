//! Independent validation of current function-relative layouts.

pub mod geometry;
mod validation;
use geometry::declared_size_matches;
use isa_aarch64::Aarch64SelectedFormEncodingError;
use isa_x86_64::X86_64SelectedFormEncodingError;
use machine_code::SelectedFormEncoding;
pub use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
};
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::MachineId;
pub use validation::{derive_order, validate_function, validate_row};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    RootMismatch,
    MixedOrdinaryAndStructuralFunctions,
    StructuralReturnRosterMismatch(SelectedInstructionId),
    StructuralEncodingMismatch(SelectedInstructionId),
    UnsupportedFunctionShape(MachineId),
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    AlternativeMismatch(SelectedInstructionId),
    UnexpectedEncodingState(SelectedInstructionId),
    OffsetOverflow,
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchSizeMismatch(SelectedInstructionId),
    OptimizationCustodyMismatch,
    OptimizationByteSavingsMismatch,
    X86_64(X86_64SelectedFormEncodingError),
    Aarch64(Aarch64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedResolvedSelectedFormLayoutError {}

/// Explicit replay relates the current layout to its admitted original owners.
pub struct ResolvedLayoutValidation<'source> {
    pub physical: &'source post_allocation_machine::PostAllocationMachineProgram,
    pub replay: post_allocation_machine::program_validation::PhysicalProgramReplay<'source>,
    pub pre_layout: &'source SelectedFormEncoding,
    pub artifact: &'source machine_code::ResolvedMachineProgram,
}

pub fn validate_optimized_resolved_selected_form_layout(
    input: ResolvedLayoutValidation<'_>,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let ResolvedLayoutValidation {
        physical,
        replay,
        pre_layout,
        artifact,
    } = input;
    post_allocation_machine::program_validation::validate_current_program(physical, replay)
        .map_err(OptimizedResolvedSelectedFormLayoutError::CurrentProgram)?;
    if pre_layout != artifact.encoding.as_ref() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    if crate::layout_requirements::prepare_layout_requirements(physical)
        != *artifact.layout_requirements
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    validation::validate(
        &artifact.layout_requirements,
        artifact.context.physical(),
        pre_layout,
        &artifact.layout,
        false,
    )
}

/// Validate the current layout, including short x86 conditional branches only
/// when the retained phase selection permits that representation.
pub fn validate_current_resolved_layout(
    program: &machine_code::ResolvedMachineProgram,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    crate::validate_encoded_integrity(&program.context, &program.frame, &program.encoding)
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::RootMismatch)?;
    if program.layout_requirements.machine != program.context.machine.identity
        || program.layout_requirements.selected != program.context.machine.selected
        || program.layout_requirements.target != program.context.target
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    let allow_x86_short = program
        .context
        .publication
        .selections()
        .contains(optimization_core::Optimization::X86RelaxConditionalBranchesToRel8V1);
    if allow_x86_short && program.context.target.architecture != target::Architecture::X86_64 {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    validation::validate(
        &program.layout_requirements,
        program.context.physical(),
        &program.encoding,
        &program.layout,
        allow_x86_short,
    )
}
