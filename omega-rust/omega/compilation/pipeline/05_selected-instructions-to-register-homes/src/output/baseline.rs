use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomes;

impl ProjectAllocation for StagedOptimizedRegisterHomes {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        allocation_output!(
            self,
            self.selected().plan(),
            self.selected(),
            AllocationEvidence::RegisterHomes(self.custody().to_owned())
        )
    }
}
