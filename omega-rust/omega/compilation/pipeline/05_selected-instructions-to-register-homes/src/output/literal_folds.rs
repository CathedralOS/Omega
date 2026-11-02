use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomesAfterSelectedLowering;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterSelectedLowering {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        let program = self.selected();
        allocation_output!(
            self,
            program.plan(),
            program,
            AllocationEvidence::SelectedLowering(self.custody().to_owned())
        )
    }
}
