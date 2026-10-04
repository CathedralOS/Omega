use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedRegisterHomesAfterPreAllocation;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterPreAllocation {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        let program = self.selected();
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: program.plan(),
                homes: self.homes().plan(),
            },
            selected: program,
            liveness: self.liveness(),
            ranges: self.ranges(),
            legality: self.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: self.register_environment(),
            target_input: self.optimized_target_owner(),
            selections: self.selections(),
            budget: self.budget_per_pass(),
            evidence: AllocationEvidence::PreAllocation(self.custody().to_owned()),
        }
    }
}
