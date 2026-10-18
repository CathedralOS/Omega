//! Check instruction spans and row-to-function fixup coordinates directly.

use super::{ResolvedFragmentEmissionError, control, require};
use machine_code::{
    FunctionFragmentBranchEvidence, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    ResolvedBranchEvidence, ResolvedSelectedFormRow, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};
use selected_instructions::{SelectedBlock, SelectedInstruction, SelectedInstructionKind};

pub(super) fn check(
    function: &selected_instructions::SelectedFunction,
    block: &SelectedBlock,
    selected: &SelectedInstruction,
    row: &ResolvedSelectedFormRow,
    span: &FunctionFragmentInstructionSpan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        span.instruction == row.instruction
            && span.alternative == row.alternative
            && span.offset == row.offset
            && span.bytes == row.bytes
            && span.provenance == selected.provenance,
    )?;
    control::check(function, block, selected, &span.control)?;
    let reference = match selected.kind {
        SelectedInstructionKind::ImmutableImageAddress { reference } => Some(reference),
        _ => None,
    };
    match (row.image_address_fixup, span.image_address_fixup, reference) {
        (None, None, None) => {}
        (Some(source), Some(actual), Some(reference)) => require(
            source.reference == reference
                && actual.reference == reference
                && source.fields.shifted(row.offset) == Some(actual.fields),
        )?,
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
    match (row.branch.as_deref(), span.branch.as_deref()) {
        (None, None) => {}
        (
            Some(ResolvedBranchEvidence::Conditional(source)),
            Some(FunctionFragmentBranchEvidence::Conditional(actual)),
        ) => {
            require(
                source.predicate == actual.predicate
                    && actual.source_block == source.source_block
                    && actual.when_taken_edge == source.when_taken_edge
                    && actual.when_taken_block == source.when_taken_block
                    && actual.when_taken_offset == source.when_taken_offset
                    && actual.when_fallthrough_edge == source.when_fallthrough_edge
                    && actual.when_fallthrough_block == source.when_fallthrough_block
                    && actual.when_fallthrough_offset == source.when_fallthrough_offset
                    && actual.byte_displacement == source.byte_displacement
                    && actual.decoded_register_reads == source.decoded_register_reads
                    && actual.decoded_effects == source.decoded_effects,
            )?;
        }
        (
            Some(ResolvedBranchEvidence::Jump(source)),
            Some(FunctionFragmentBranchEvidence::Jump(actual)),
        ) => {
            require(
                actual.source_block == source.source_block
                    && actual.target_edge == source.target_edge
                    && actual.target_block == source.target_block
                    && actual.target_offset == source.target_offset
                    && actual.byte_displacement == source.byte_displacement
                    && actual.decoded_effects == source.decoded_effects,
            )?;
        }
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
    match (&row.internal_machine_fixup, &span.internal_machine_fixup, selected.kind) {
        (Some(source), Some(actual), selected_instructions::selected_call_triple_pattern!(target:selected_instructions::SelectedCallTarget::Machine(callee), ..)) => {
            require(row.branch.is_none() && source.callee == callee && actual.callee == callee
                && source.state == SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                && actual.state == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
                && matches!((source.kind, actual.kind),
                    (SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                     FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1)
                    | (SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                       FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1))
                && Some(actual.opcode_function_offset) == row.offset.checked_add(u64::from(source.opcode_row_offset))
                && Some(actual.patch_function_offset) == row.offset.checked_add(u64::from(source.patch_row_offset))
                && Some(actual.reference_function_offset) == row.offset.checked_add(u64::from(source.reference_row_offset))
                && actual.patch_byte_width == source.patch_byte_width && actual.addend == source.addend)
        },
        (None, None, kind) if !matches!(kind, selected_instructions::selected_call_triple_pattern!(target:selected_instructions::SelectedCallTarget::Machine(_), ..)) => Ok(()),
        _ => Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
}
