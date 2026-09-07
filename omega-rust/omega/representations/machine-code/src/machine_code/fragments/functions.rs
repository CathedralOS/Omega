//! Functions, blocks, and instruction spans in an unplaced fragment.

use super::{FunctionFragmentControlProvenance, FunctionFragmentInternalMachineFixup};
use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionProvenance,
};
use semantic_vocabulary::MachineId;
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub blocks: Vec<FunctionFragmentBlockSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentBlockSpan {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<FunctionFragmentInstructionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentInstructionSpan {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<super::FunctionFragmentBranchEvidence>>,
    pub internal_machine_fixup: Option<FunctionFragmentInternalMachineFixup>,
    pub provenance: SelectedInstructionProvenance,
    pub control: FunctionFragmentControlProvenance,
}
