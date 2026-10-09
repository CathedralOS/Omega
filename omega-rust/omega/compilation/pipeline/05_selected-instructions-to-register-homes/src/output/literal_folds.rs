use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterLiteralFolds {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(self, self.selected().transformed(), SelectedProgramRef::new(self.selected()), AllocationEvidence::LiteralFolds(self.custody().to_owned()))
    }
}

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterSelectedLowering {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        let program = self.selected();
        allocation_output!(self, program.plan(), program, AllocationEvidence::SelectedLowering(self.custody().to_owned()))
    }
}
