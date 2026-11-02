use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedActiveResidentRematerialization;
use selected_instruction_analyses::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedActiveResidentRematerialization {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(
            self,
            self.rematerialization().transformed(),
            SelectedProgramRef::new(self.rematerialization()),
            AllocationEvidence::ActiveResidentRematerialization(self.custody().to_owned(),)
        )
    }
}
