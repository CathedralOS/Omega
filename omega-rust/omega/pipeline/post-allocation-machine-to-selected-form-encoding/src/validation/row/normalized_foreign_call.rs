use isa_aarch64::{
    Aarch64NormalizedForeignCallFixup, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState,
    validate_aarch64_selected_normalized_foreign_call_template,
};
use isa_x86_64::{
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState,
    validate_x86_64_selected_normalized_foreign_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::{decoded_footprint, operand_views, validate_machine_footprint, validate_size};
use crate::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingState,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

/// Independently validate one unresolved normalized-foreign-call row. The
/// validator re-derives the canonical template from the machine row and the
/// kind's `{boundary, ordinal}` binding; a substituted boundary, ordinal,
/// byte, fixup, or effect cannot self-certify.
pub(super) fn validate(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
        bytes,
        footprint,
        fixup,
    } = state
    else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.addend != 0
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
    };
    if fixup.boundary != boundary || fixup.ordinal != ordinal {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let views = operand_views(machine);
    let accesses = machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect::<Vec<_>>();
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let validated = validate_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                X86_64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                },
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &views_for_effects(machine, validated.effects(), true)?,
                &views_for_effects(machine, validated.effects(), false)?,
                validated.effects(),
            )
        }
        Architecture::Aarch64 => {
            let target_kind = match fixup.kind {
                SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let validated = validate_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
                bytes,
                Aarch64NormalizedForeignCallFixup {
                    kind: target_kind,
                    state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                    boundary: fixup.boundary,
                    ordinal: fixup.ordinal,
                    opcode_byte_offset: fixup.opcode_row_offset,
                    patch_byte_offset: fixup.patch_row_offset,
                    reference_byte_offset: fixup.reference_row_offset,
                    patch_byte_width: fixup.patch_byte_width,
                },
            )
            .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
            decoded_footprint(
                &views_for_effects(machine, validated.effects(), true)?,
                &views_for_effects(machine, validated.effects(), false)?,
                validated.effects(),
            )
        }
    };
    validate_machine_footprint(instruction, machine, &decoded)?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    if footprint.as_ref() != &decoded {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn views_for_effects(
    machine: &PostAllocationMachineInstruction,
    effects: &selected_instructions::MachineEncodedEffects,
    reads: bool,
) -> Result<Vec<register_model::RegisterViewId>, OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let operands = if reads {
        &effects.external_operand_reads
    } else {
        &effects.external_operand_writes
    };
    operands
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))
}
