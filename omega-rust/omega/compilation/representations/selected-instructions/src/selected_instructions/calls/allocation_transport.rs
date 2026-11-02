//! Recognize compiler-private allocation steps inside retained call transport.
//!
//! These copies and spill reloads have no source operation, external effect or
//! authority. Selected allocation and physical replay own their value/storage
//! derivation; call custody still rejoins the exact destination and source.
use crate::FrameStorageSlotId::Local;
use crate::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedInstructionKind,
    VirtualRegisterOrigin,
};
use register_model::RegisterOperandAccess;

/// Whether this selected step is private copy/reload transport. It cannot
/// address an outgoing argument or a boundary's control/result storage.
pub fn is_private_step(
    function: &SelectedFunction,
    block: &SelectedBlock,
    position: usize,
) -> bool {
    let Some(instruction) = block.instructions.get(position) else {
        return false;
    };
    let provenance = &instruction.provenance;
    if !provenance.operations.is_empty()
        || !provenance.edges.is_empty()
        || !provenance.obligations.is_empty()
        || !provenance.fuel.is_empty()
    {
        return false;
    }
    match instruction.kind {
        SelectedInstructionKind::CopyI64 => {
            let [input, output] = instruction.operands.as_slice() else {
                return false;
            };
            let (Some(input_value), Some(output_value)) = (
                function.virtual_register(input.virtual_register),
                function.virtual_register(output.virtual_register),
            ) else {
                return false;
            };
            input.access == RegisterOperandAccess::Use
                && output.access == RegisterOperandAccess::Def
                && input_value.scalar_type == output_value.scalar_type
                && input_value.class == output_value.class
                && instruction.implicit_uses.is_empty()
                && instruction.implicit_defs.is_empty()
                && instruction.clobbers.is_empty()
                && instruction.operands.iter().all(|operand| {
                    operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber
                })
        }
        SelectedInstructionKind::FrameAddress {
            slot:
                Local(LocalStorageSlotId::Spill {
                    register: slot_register,
                }),
            byte_offset: 0,
        } => {
            let [output] = instruction.operands.as_slice() else {
                return false;
            };
            provenance.values.is_empty()
                && output.access == RegisterOperandAccess::Def
                && function
                    .virtual_register(output.virtual_register)
                    .is_some_and(|value| {
                        // Slot reuse separates storage identity from the value's lineage.
                        matches!(value.origin, VirtualRegisterOrigin::SpillAddress {
                            instruction: address_instruction, ..
                        } if address_instruction == instruction.id)
                    })
                && function.local_storage_slots.iter().any(|home| {
                    home.id
                        == LocalStorageSlotId::Spill {
                            register: slot_register,
                        }
                        && home.byte_size == 8
                        && home.alignment == 8
                })
        }
        SelectedInstructionKind::Load64 { byte_offset: 0 } => {
            let [address, output] = instruction.operands.as_slice() else {
                return false;
            };
            let Some(value) = function.virtual_register(address.virtual_register) else {
                return false;
            };
            let VirtualRegisterOrigin::SpillAddress {
                instruction: address_instruction,
                ..
            } = value.origin
            else {
                return false;
            };
            let Some(address_position) = block.instruction_index(address_instruction) else {
                return false;
            };
            provenance.values.is_empty()
                && address.access == RegisterOperandAccess::Use
                && output.access == RegisterOperandAccess::Def
                && instruction.implicit_uses.is_empty()
                && instruction.implicit_defs.is_empty()
                && instruction.clobbers.is_empty()
                && address_position < position
                && block.instructions[address_position]
                    .operands
                    .first()
                    .is_some_and(|output| output.virtual_register == address.virtual_register)
                && is_private_step(function, block, address_position)
        }
        _ => false,
    }
}
