use isa_aarch64::{
    Aarch64NormalizedForeignCallFixupKind, Aarch64NormalizedForeignCallFixupState,
    encode_aarch64_selected_normalized_foreign_call_template,
};
use isa_x86_64::{
    X86_64NormalizedForeignCallFixupKind, X86_64NormalizedForeignCallFixupState,
    encode_x86_64_selected_normalized_foreign_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::{validate_operand_footprint, validate_size};
use crate::{
    OptimizedSelectedFormEncodingError, SelectedFormDecodedFootprint, SelectedFormEncodingState,
    SelectedFormNormalizedForeignCallFixup, SelectedFormNormalizedForeignCallFixupKind,
    SelectedFormNormalizedForeignCallFixupState,
};

/// Encode one normalized foreign call row into the canonical unresolved
/// import-field template. The fixup carries the selected `{boundary,
/// ordinal}` roster binding; the byte field stays a canonical zero/immediate
/// placeholder until object construction names the import symbol.
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
    let accesses = machine
        .operands
        .iter()
        .map(|operand| operand.access)
        .collect::<Vec<_>>();
    let (bytes, effects, fixup) = match target.architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::X86_64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormNormalizedForeignCallFixup {
                    kind,
                    state,
                    boundary: target_fixup.boundary,
                    ordinal: target_fixup.ordinal,
                    opcode_row_offset: target_fixup.opcode_byte_offset,
                    patch_row_offset: target_fixup.patch_byte_offset,
                    reference_row_offset: target_fixup.reference_byte_offset,
                    patch_byte_width: target_fixup.patch_byte_width,
                    addend: 0,
                },
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_selected_normalized_foreign_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &accesses,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::Aarch64NormalizedForeignCall)?;
            let target_fixup = encoded.fixup();
            let kind = match target_fixup.kind {
                Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                    SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
                }
            };
            let state = match target_fixup.state {
                Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1 => {
                    SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
                }
            };
            (
                encoded.bytes().to_vec(),
                encoded.effects().clone(),
                SelectedFormNormalizedForeignCallFixup {
                    kind,
                    state,
                    boundary: target_fixup.boundary,
                    ordinal: target_fixup.ordinal,
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
    Ok(SelectedFormEncodingState::UnresolvedNormalizedForeignCall {
        bytes,
        footprint: Box::new(footprint),
        fixup,
    })
}

fn decoded_footprint(
    machine: &PostAllocationMachineInstruction,
    effects: &selected_instructions::MachineEncodedEffects,
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
