//! Validate current machine encodings and replay explicitly retained originals.
//!
//! Frame equations and contract projections are shared with the producer, as
//! before this split. Byte validation uses the independent ISA decoders; call
//! template comparison retains its existing shared-recipe limitation. No
//! producer stage or callback is an input to these checks.

#![forbid(unsafe_code)]
mod control_encoding;
pub mod exit_requirements;
pub mod frame_address;
pub mod frame_protocol;
pub mod layout;
pub mod layout_requirements;
pub mod relative_branches;
pub mod relaxation;
mod validation;
use OptimizedSelectedFormEncodingError as Error;
use isa_aarch64::{
    Aarch64NormalizedForeignCallTemplateError, Aarch64ScalarCallTemplateError,
    Aarch64SelectedFormEncodingError,
};
use isa_x86_64::{
    X86_64MovR32Imm32I64MaterializationError, X86_64MovR64Imm32SignExtendedI64MaterializationError,
    X86_64NormalizedForeignCallTemplateError, X86_64ScalarCallTemplateError,
    X86_64SelectedFormEncodingError,
};
use machine_code::{
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, TargetFrameLayoutPlan,
};
use post_allocation_machine::{
    PostAllocationMachineFunction, PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedNormalizedForeignCall, terminator_instruction,
};
use std::sync::Arc;
use target::NativeTarget;
pub use validation::{validate_row, validate_size};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedFormEncodingError {
    ExitRequirements(machine_code::exit_requirements::WholeFunctionExitContractError),
    CurrentProgram(post_allocation_machine::program_validation::PhysicalProgramValidationError),
    SelectedRootMismatch,
    PhysicalModelMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    OperandFootprintMismatch(SelectedInstructionId),
    ImplicitFootprintMismatch(SelectedInstructionId),
    SizeDeclarationMismatch(SelectedInstructionId),
    CountOverflow,
    X86_64(X86_64SelectedFormEncodingError),
    X86_64ScalarCall(X86_64ScalarCallTemplateError),
    X86_64NormalizedForeignCall(X86_64NormalizedForeignCallTemplateError),
    X86_64MovR32Imm32(X86_64MovR32Imm32I64MaterializationError),
    X86_64MovR64Imm32SignExtended(X86_64MovR64Imm32SignExtendedI64MaterializationError),
    Aarch64(Aarch64SelectedFormEncodingError),
    Aarch64ScalarCall(Aarch64ScalarCallTemplateError),
    Aarch64NormalizedForeignCall(Aarch64NormalizedForeignCallTemplateError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedFormEncodingError {}

/// Independent replay borrows the original admitted owners separately from
/// the current encoding. Ordinary encoding never receives these owners.
pub struct SelectedFormEncodingValidation<'source> {
    pub physical: &'source post_allocation_machine::PostAllocationMachineProgram,
    pub replay: post_allocation_machine::program_validation::PhysicalProgramReplay<'source>,
    pub frame: Option<&'source TargetFrameLayoutPlan>,
    pub artifact: &'source machine_code::EncodedMachineProgram,
}

pub fn validate_optimized_layout_independent_selected_form_encoding(
    input: SelectedFormEncodingValidation<'_>,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let SelectedFormEncodingValidation {
        physical,
        replay,
        frame,
        artifact,
    } = input;
    post_allocation_machine::program_validation::validate_current_program(physical, replay)
        .map_err(OptimizedSelectedFormEncodingError::CurrentProgram)?;
    if program_context(
        physical,
        Arc::clone(&artifact.frame_demands),
        artifact.encoding.identity(),
    ) != *artifact.context
        || layout_requirements::prepare_layout_requirements(physical)
            != *artifact.layout_requirements
        || exit_requirements::prepare_exit_requirements(physical, &artifact.encoding)
            .map_err(OptimizedSelectedFormEncodingError::ExitRequirements)?
            != *artifact.exit_requirements
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    if frame != Some(&*artifact.frame) {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    validate_current_selected_form_encoding(physical, &artifact.frame, &artifact.encoding)
}

/// Check encoded bytes against the current instruction, physical and frame
/// columns. Comparing them with original allocation inputs is a separate audit.
pub fn validate_current_selected_form_encoding(
    program: &post_allocation_machine::PostAllocationMachineProgram,
    frame: &TargetFrameLayoutPlan,
    encoding: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    post_allocation_machine::program_validation::validate_current_program_integrity(program)
        .map_err(OptimizedSelectedFormEncodingError::CurrentProgram)?;
    validation::validate(program, Some(frame), encoding)
}

pub fn normalized_foreign_call_record(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    foreign_calls: &[SelectedNormalizedForeignCall],
) -> Result<&SelectedNormalizedForeignCall, Error> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(Error::ArtifactMismatch);
    };
    let record = foreign_calls
        .get(ordinal as usize)
        .ok_or(Error::ArtifactMismatch)?;
    if record.instruction != instruction
        || record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || !record.call.binding.has_valid_entry_mechanism()
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(record)
}

pub(crate) fn for_each_row<V>(
    selected: &post_allocation_machine::PostAllocationMachineProgram,
    machine: &PostAllocationMachinePlan,
    mut visit: V,
) -> Result<(), OptimizedSelectedFormEncodingError>
where
    V: FnMut(
        &SelectedFunction,
        &SelectedInstruction,
        &PostAllocationMachineFunction,
        &PostAllocationMachineInstruction,
    ) -> Result<(), OptimizedSelectedFormEncodingError>,
{
    let selected_plan = selected;
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    for (selected_function, machine_function) in
        selected_plan.functions.iter().zip(&machine.functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (selected_block, machine_block) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                visit(
                    selected_function,
                    selected_instruction,
                    machine_function,
                    machine_instruction,
                )?;
            }
        }
    }
    Ok(())
}

pub fn counts_of(
    rows: &[SelectedFormEncodingRow],
) -> Result<SelectedFormEncodingCounts, OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                counts.ordinary_deferred_internal_control = counts
                    .ordinary_deferred_internal_control
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_internal_fixups
            }
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { .. } => {
                counts.ordinary_encoded_call_templates = counts
                    .ordinary_encoded_call_templates
                    .checked_add(1)
                    .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
                &mut counts.ordinary_normalized_foreign_import_fixups
            }
            SelectedFormEncodingState::UnresolvedImageAddress { .. } => {
                &mut counts.ordinary_image_address_fixups
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    Ok(counts)
}

/// The checked encoding identity and immutable target roots are sufficient for
/// current byte/layout checks; source correspondence takes explicit originals.
pub fn validate_encoded_integrity(
    context: &machine_code::program_context::EncodedProgramContext,
    frame: &TargetFrameLayoutPlan,
    encoding: &SelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    if encoding.identity() != context.checked_encoding
        || encoding.recomputed_identity() != context.checked_encoding
        || encoding.machine() != context.machine.identity
        || encoding.selected() != context.machine.selected
        || encoding.frame.as_ref() != Some(frame)
        || frame.post_allocation_machine != context.machine.identity
        || frame.register_environment != context.environment.identity()
        || frame.physical_register_model != context.physical().identity()
        || frame.target != context.target
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}
pub fn program_context(
    program: &post_allocation_machine::PostAllocationMachineProgram,
    machine: Arc<machine_code::frame_demands::MachineFrameDemands>,
    checked_encoding: SelectedFormEncodingIdentity,
) -> machine_code::program_context::EncodedProgramContext {
    machine_code::program_context::EncodedProgramContext {
        contracts: Arc::clone(&program.contracts),
        psi: program.psi,
        work_schedule: program.work_schedule,
        entry: program.entry,
        machine,
        effects: program.machine.effects,
        environment: program.environment.clone(),
        preservation: program.preservation.clone(),
        publication: Arc::clone(&program.publication),
        phase: program.phase,
        identity: program.identity,
        checked_encoding,
    }
}

pub mod fragment_origins;
