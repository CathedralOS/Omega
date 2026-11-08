/*
  Virtual registers, origin provenance (`VirtualRegisterOrigin`), and
  instruction operand constraints (`SelectedOperand`) prior to physical
  register assignment.
*/

use super::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
use semantic_vocabulary::{PlaceId, ScalarType, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegister {
    pub id: VirtualRegisterId,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: Option<ValueDefinitionSite>,
    pub entry_fixed_view: Option<RegisterViewId>, // An ABI live-in constraint, not an assigned physical home.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualRegisterOrigin {
    InstructionScratch {
        instruction: SelectedInstructionId,
        operand: u16,
    },
    StructuralObservation {
        instruction: SelectedInstructionId,
        place: PlaceId,
        byte_offset: u32,
    },
    ScalarAbiAddress {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
    SpillAddress {
        instruction: SelectedInstructionId,
        register: VirtualRegisterId,
    },
    StructuralParameter {
        place: PlaceId,
        parameter_index: usize,
    },
    AbiTransport {
        instruction: SelectedInstructionId,
        place: PlaceId,
        byte_offset: u32,
    },
    EntryParameter {
        source_value: ValueId,
        parameter_index: usize,
    },
    BlockParameter {
        source_value: ValueId,
        block: SelectedBlockId,
        parameter_index: usize,
    },
    InstructionResult {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
}

impl VirtualRegisterOrigin {
    pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
        match self {
            VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand,
            } => {
                bytes.push(9);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, operand.to_le_bytes());
            }
            VirtualRegisterOrigin::StructuralObservation {
                instruction,
                place,
                byte_offset,
            } => {
                bytes.push(8);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            }
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction,
                source_value,
            } => {
                bytes.push(7);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            }
            VirtualRegisterOrigin::SpillAddress {
                instruction,
                register,
            } => {
                bytes.push(6);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
            }
            VirtualRegisterOrigin::StructuralParameter {
                place,
                parameter_index,
            } => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, (*parameter_index as u64).to_le_bytes());
            }
            VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            } => {
                bytes.push(5);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            }
            VirtualRegisterOrigin::BlockParameter {
                source_value,
                block,
                parameter_index,
            } => {
                bytes.push(3);
                crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, block.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, (*parameter_index as u64).to_le_bytes());
            }
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index,
            } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, (*parameter_index as u64).to_le_bytes());
            }
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedOperand {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>, // Fixed instruction-use/def constraint, not an assigned home.
    pub tied_to: Option<u16>, // Canonical one-way allocation tie to an earlier operand.
    pub early_clobber: bool,  // This definition may clobber before unrelated inputs are all read.
}
