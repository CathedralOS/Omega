use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::StagedOptimizedActiveResidentRematerialization;
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedActiveResidentRematerialization {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: self.rematerialization().transformed(),
                homes: self.homes().plan(),
            },
            selected: SelectedProgramRef::new(self.rematerialization()),
            liveness: self.liveness(),
            ranges: self.ranges(),
            legality: self.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: self.register_environment(),
            target_input: self.optimized_target_owner(),
            selections: self.selections(),
            budget: self.budget_per_pass(),
            evidence: AllocationEvidence::ActiveResidentRematerialization(
                self.custody().to_owned(),
            ),
        }
    }
}
