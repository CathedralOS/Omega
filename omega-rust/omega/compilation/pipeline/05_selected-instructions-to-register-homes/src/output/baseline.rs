use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomes;
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedRegisterHomes {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(self, self.selected().plan(), SelectedProgramRef::new(self.selected()), AllocationEvidence::RegisterHomes(self.custody().to_owned()))
    }
}
