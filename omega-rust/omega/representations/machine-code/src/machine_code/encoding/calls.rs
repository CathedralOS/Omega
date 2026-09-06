//! Structural call templates and their value-less function return rows.

use super::SelectedFormEncodingRow;
use crate::{X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup};
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::{MachineId, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitCallEncodingRow {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitFunctionEncoding {
    pub machine: MachineId,
    pub block: selected_instructions::SelectedBlockId,
    pub call: Option<SelectedStructuralUnitCallEncodingRow>,
    pub return_instruction: SelectedFormEncodingRow,
}
