//! Virtual values and instruction operand constraints; no assigned storage.
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
    /// An ABI live-in constraint. This is not an assigned physical home.
    pub entry_fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualRegisterOrigin {
    /// Instruction-local temporary, never a source value or structural place.
    InstructionScratch {
        instruction: SelectedInstructionId,
        operand: u16,
    },
    /// A compiler-owned structural address or loaded observation, not a new
    /// Terminal scalar definition. The instruction distinguishes the operation.
    StructuralObservation {
        instruction: SelectedInstructionId,
        place: PlaceId,
        byte_offset: u32,
    },
    /// Compiler-owned ABI stack address associated with a scalar source, not its payload.
    ScalarAbiAddress {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
    /// Compiler-owned frame address for one original virtual value's spill slot.
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
    /// Defined by the selected block's incoming edge bindings, not by any
    /// instruction in a predecessor. Each edge retains its distinct argument.
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
    /// The canonical identity encoding shared by every identity vocabulary
    /// that fingerprints a register origin (plan identity, wire codec,
    /// recovery classification).
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
                crate::codec_bytes::push_encoded(
                    bytes,
                    (*parameter_index as u64).to_le_bytes(),
                );
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
                crate::codec_bytes::push_encoded(
                    bytes,
                    (*parameter_index as u64).to_le_bytes(),
                );
            }
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index,
            } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
                crate::codec_bytes::push_encoded(
                    bytes,
                    (*parameter_index as u64).to_le_bytes(),
                );
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
    /// A fixed instruction-use/def constraint, not an assigned home.
    pub fixed_view: Option<RegisterViewId>,
    /// Canonical one-way allocation tie to an earlier operand.
    pub tied_to: Option<u16>,
    /// This definition may clobber before unrelated inputs are all read.
    pub early_clobber: bool,
}
