//! Function and block spans with their ordered encoded instruction rows.

use omega_selected_instructions::{MachineAlternativeKey, SelectedBlockId, SelectedInstructionId};
use psi_core::MachineId;

use super::ResolvedConditionalBranchEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFormRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<ResolvedConditionalBranchEvidence>>,
    pub internal_machine_fixup: Option<crate::SelectedFormInternalMachineFixup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedBlockLayout {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<ResolvedSelectedFormRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFunctionLayout {
    pub machine: MachineId,
    pub byte_count: u64,
    pub blocks: Vec<ResolvedSelectedBlockLayout>,
}
