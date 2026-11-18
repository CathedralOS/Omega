//! Branch correspondence retained with framed bytes, without earlier fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFrameBranchOrigins {
    pub machine: semantic_vocabulary::MachineId,
    pub branches: Vec<FrameBranchOrigin>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBranchOrigin {
    pub instruction: selected_instructions::SelectedInstructionId,
    pub branch: crate::FunctionFragmentBranchEvidence,
}
