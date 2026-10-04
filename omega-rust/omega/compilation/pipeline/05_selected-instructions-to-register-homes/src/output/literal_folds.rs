use super::{AllocationEvidence, AllocationOutput, ProjectAllocation};
use crate::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterLiteralFolds {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: self.selected().transformed(),
                homes: self.homes().plan(),
            },
            selected: SelectedProgramRef::new(self.selected()),
            liveness: self.liveness(),
            ranges: self.ranges(),
            legality: self.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: self.register_environment(),
            target_input: self.optimized_target_owner(),
            selections: self.selections(),
            budget: self.budget_per_pass(),
            evidence: AllocationEvidence::LiteralFolds(self.custody().to_owned()),
        }
    }
}

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterSelectedLowering {
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
            evidence: AllocationEvidence::SelectedLowering(self.custody().to_owned()),
        }
    }
}
