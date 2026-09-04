use omega_isa_aarch64::{
    Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    encode_aarch64_selected_scalar_call_template,
};
use omega_isa_x86_64::{
    X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    encode_x86_64_selected_scalar_call_template,
};
use omega_machine_optimizer::PostAllocationMachineInstruction;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use omega_target::{Architecture, NativeTarget};

use super::{validate_operand_footprint, validate_size};
use crate::{
    OptimizedSelectedFormEncodingError, SelectedFormDecodedFootprint, SelectedFormEncodingState,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState,
};

pub(super) fn encode(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let (bytes, effects, fixup) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::X86_64ScalarCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1 => {
                    SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
                }
            };
            let state = match target_fixup.state {
                X86_64ScalarCallFixupState::UnresolvedZeroFieldV1 => {
                    SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormInternalMachineFixup {
                    kind,
                    state,
                    callee: target_fixup.callee,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                },
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_selected_scalar_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::Aarch64ScalarCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1 => {
                    SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
                }
            };
            let state = match target_fixup.state {
                Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1 => {
                    SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormInternalMachineFixup {
                    kind,
                    state,
                    callee: target_fixup.callee,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                },
            )
        }
    };
    let footprint = decoded_footprint(machine, &effects)?;
    validate_operand_footprint(
        instruction,
        machine,
        &effects,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodingState::UnresolvedInternalMachineCall {
        bytes,
        footprint: Box::new(footprint),
        fixup,
    })
}

fn decoded_footprint(
    machine: &PostAllocationMachineInstruction,
    effects: &omega_selected_instructions::MachineEncodedEffects,
) -> Result<SelectedFormDecodedFootprint, OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let register_reads = effects
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))?;
    let register_writes = effects
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(machine.instruction))?;
    Ok(SelectedFormDecodedFootprint {
        register_reads,
        register_writes,
        implicit_defs: effects.implicit_unit_defs.clone(),
        implicit_clobbers: effects.implicit_unit_clobbers.clone(),
        encoded: effects.clone(),
    })
}
