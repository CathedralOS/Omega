//! Function and block spans with their ordered encoded instruction rows.

use semantic_vocabulary::MachineId;
use target_operations_to_selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
};

use super::ResolvedBranchEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedFormRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<ResolvedBranchEvidence>>,
    pub internal_machine_fixup: Option<crate::machine_code::SelectedFormInternalMachineFixup>,
    /// Unresolved normalized-foreign-call field carried through resolved
    /// layout; object construction binds it to the declared import symbol.
    pub normalized_foreign_call_fixup:
        Option<crate::machine_code::SelectedFormNormalizedForeignCallFixup>,
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
