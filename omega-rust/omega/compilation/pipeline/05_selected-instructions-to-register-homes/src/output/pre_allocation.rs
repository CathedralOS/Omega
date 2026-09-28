use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    ProjectAllocation, sealed,
};
use crate::{
    StagedOptimizedRegisterHomesAfterPreAllocation,
    validate_optimized_register_home_after_pre_allocation_custody,
};

impl sealed::Sealed for StagedOptimizedRegisterHomesAfterPreAllocation {}

impl AllocationSource for StagedOptimizedRegisterHomesAfterPreAllocation {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_register_home_after_pre_allocation_custody(self)
            .map_err(AllocationReplayError::PreAllocation)?;
        if &evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        Ok(self.project_allocation())
    }
}

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
