//! Check instruction spans and row-to-function fixup coordinates directly.

use super::{ResolvedFragmentEmissionError, control, require};
use ::machine_code::*;
use selected_instructions::{SelectedBlock, SelectedInstruction, SelectedInstructionKind};

pub(super) fn check(
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
    control::check(block, selected, &span.control)?;
    match (row.branch.as_deref(), span.branch.as_deref()) {
        (None, None) => {}
        (
            Some(ResolvedBranchEvidence::Conditional(source)),
            Some(FunctionFragmentBranchEvidence::Conditional(actual)),
        ) => {
            require(
                matches!(
                    (source.predicate, actual.predicate),
                    (
                        ResolvedConditionalBranchPredicate::NonZeroV1,
                        FunctionFragmentConditionalBranchPredicate::NonZeroV1
                    ) | (
                        ResolvedConditionalBranchPredicate::U64LessThanV1,
                        FunctionFragmentConditionalBranchPredicate::U64LessThanV1
                    ) | (
                        ResolvedConditionalBranchPredicate::I64LessThanV1,
                        FunctionFragmentConditionalBranchPredicate::I64LessThanV1
                    )
                ) && actual.source_block == source.source_block
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
        (Some(source), Some(actual), SelectedInstructionKind::CallI64 { callee }) => {
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
        (None, None, kind) if !matches!(kind, SelectedInstructionKind::CallI64 { .. }) => Ok(()),
        _ => Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
}
