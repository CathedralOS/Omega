//! Attribution and relocation obligations carried into byte placement.
use crate::FunctionFragmentControlProvenance;
use selected_instructions::{
    SelectedBlockId, SelectedImageAddressReference, SelectedInstructionId,
    SelectedInstructionProvenance,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId, WorkScheduleIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFragmentEmissionError {
    MissingFunction(MachineId),
    MissingBlock(selected_instructions::SelectedBlockId),
    MissingInstruction(selected_instructions::SelectedInstructionId),
    OffsetOverflow,
    RootMismatch,
    ArtifactMismatch,
}

impl std::fmt::Display for ResolvedFragmentEmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolved fragment projection failed: {self:?}")
    }
}

impl std::error::Error for ResolvedFragmentEmissionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentOrigins {
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: std::sync::Arc<selected_instructions::SelectedInstructionPlan>,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<FragmentFunctionOrigin>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentFunctionOrigin {
    pub entry_block: SelectedBlockId,
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub provenance: target_vocabulary::TerminalPsiProvenance,
    pub blocks: Vec<FragmentBlockOrigin>,
}
impl FragmentFunctionOrigin {
    pub fn block(&self, id: SelectedBlockId) -> Option<&FragmentBlockOrigin> {
        self.blocks.iter().find(|b| b.id == id)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentBlockOrigin {
    pub id: SelectedBlockId,
    pub instructions: Vec<FragmentInstructionOrigin>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentInstructionOrigin {
    pub id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub control: Result<FunctionFragmentControlProvenance, ResolvedFragmentEmissionError>,
    pub callee: Option<MachineId>,
    pub foreign: Option<(BoundaryMachineId, u32)>,
    pub image_reference: Option<SelectedImageAddressReference>,
    pub import_free: Result<bool, ResolvedFragmentEmissionError>,
}
