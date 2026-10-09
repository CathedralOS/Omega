use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomesAfterFixedViewCopies;
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterFixedViewCopies {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(self, &self.selected().plan().transformed, SelectedProgramRef::new(self.selected()), AllocationEvidence::FixedViewCopies(self.custody().to_owned()))
    }
}
