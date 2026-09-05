use isa_aarch64::{
    Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    validate_aarch64_selected_scalar_call_template,
};
use isa_x86_64::{
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    validate_x86_64_selected_scalar_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::{decoded_footprint, operand_views, validate_machine_footprint, validate_size};
use crate::selected_form_encoding::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingState,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
};

pub(super) fn validate(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    state: &SelectedFormEncodingState,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let SelectedFormEncodingState::UnresolvedInternalMachineCall {
        bytes,
        footprint,
        fixup,
    } = state
    else {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    };
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.addend != 0
    {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let views = operand_views(machine);
    let decoded = match target.architecture {
        Architecture::X86_64 => {
            let target_kind = match fixup.kind {
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => {
                    X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let validated = validate_x86_64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
                bytes,
                X86_64ScalarCallFixup {
                    kind: target_kind,
                    state: X86_64ScalarCallFixupState::UnresolvedZeroFieldV1,
                    callee: fixup.callee,
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
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => {
                    Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1
                }
                _ => return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch),
            };
            let validated = validate_aarch64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
                bytes,
                Aarch64ScalarCallFixup {
                    kind: target_kind,
                    state: Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1,
                    callee: fixup.callee,
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
    let operands = if reads {
        &effects.external_operand_reads
    } else {
        &effects.external_operand_writes
    };
    operands
        .iter()
        .map(|operand| {
            machine
                .operands
                .iter()
                .find(|row| row.operand == *operand)
                .map(|row| row.view)
                .ok_or(
                    OptimizedSelectedFormEncodingError::OperandFootprintMismatch(
                        machine.instruction,
                    ),
                )
        })
        .collect()
}
