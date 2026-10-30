use isa_aarch64::{
    Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    encode_aarch64_selected_scalar_call_template,
};
use isa_x86_64::{
    X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    encode_x86_64_selected_scalar_call_template,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedInstructionId, SelectedInstructionKind};
use target::{Architecture, NativeTarget};

use super::SelectedFormEncodedRowDraft;
use crate::OptimizedSelectedFormEncodingError;
use machine_code::{
    SelectedFormEncodingState, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
};

pub(super) fn encode(
    target: NativeTarget,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodedRowDraft, OptimizedSelectedFormEncodingError> {
    let views = super::operand_views(machine);
    if matches!(
        kind.call_target(),
        Some(selected_instructions::SelectedCallTarget::Dynamic { .. })
    ) {
        let (bytes, effects) = match target.architecture {
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_selected_dynamic_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::Aarch64ScalarCall)?,
            Architecture::X86_64 => isa_x86_64::encode_x86_64_selected_dynamic_call_template(
                target,
                physical,
                kind,
                machine.alternative.key,
                &views,
                &machine.alternative.encoded,
            )
            .map_err(OptimizedSelectedFormEncodingError::X86_64ScalarCall)?,
        };
        return super::validated_encoded_draft(
            instruction,
            machine,
            &effects,
            bytes,
            SelectedFormEncodingState::Encoded,
        );
    }
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
    super::validated_encoded_draft(
        instruction,
        machine,
        &effects,
        bytes,
        SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
    )
}
