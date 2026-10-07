//! Unresolved addresses of complete implementation tables. Selection owns
//! no receiver: only its table address flows through ordinary scalar storage.
//! Fields move from row to function to section coordinates without patching;
//! image construction binds them to the exact catalog table.

use crate::ByteLiteralAddressFields;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementationTableReference {
    pub table: u32,
}

impl ImplementationTableReference {
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.table.to_le_bytes());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormImplementationTableAddressFixup {
    pub reference: ImplementationTableReference,
    pub fields: ByteLiteralAddressFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentImplementationTableAddressFixup {
    pub reference: ImplementationTableReference,
    pub fields: ByteLiteralAddressFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedImplementationTableAddress {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub reference: ImplementationTableReference,
    pub operation: OperationId,
    pub instruction_function_offset: u64,
    pub instruction_section_offset: u64,
    pub instruction_byte_count: u64,
    pub fields: ByteLiteralAddressFields,
}

impl PlacedImplementationTableAddress {
    pub fn encode_identity(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.machine.get().to_le_bytes());
        bytes.extend_from_slice(&self.block.0.to_le_bytes());
        bytes.extend_from_slice(&self.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&self.reference.table.to_le_bytes());
        bytes.extend_from_slice(&self.operation.get().to_le_bytes());
        bytes.extend_from_slice(&self.instruction_function_offset.to_le_bytes());
        bytes.extend_from_slice(&self.instruction_section_offset.to_le_bytes());
        bytes.extend_from_slice(&self.instruction_byte_count.to_le_bytes());
        self.fields.encode_identity(bytes);
    }
}
