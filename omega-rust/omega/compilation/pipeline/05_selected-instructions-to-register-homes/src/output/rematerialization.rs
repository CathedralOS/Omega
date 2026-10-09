use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedActiveResidentRematerialization;
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedActiveResidentRematerialization {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(self, self.rematerialization().transformed(), SelectedProgramRef::new(self.rematerialization()), AllocationEvidence::ActiveResidentRematerialization(self.custody().to_owned(),))
    }
}
