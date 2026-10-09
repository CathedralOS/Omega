use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomesAfterPreAllocation;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterPreAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        let program = self.selected();
        allocation_output!(self, program.plan(), program, AllocationEvidence::PreAllocation(self.custody().to_owned()))
    }
}
