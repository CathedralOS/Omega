use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    ProjectAllocation, sealed,
};
use crate::SelectedProgramRef;
use crate::{StagedOptimizedRegisterHomes, validate_optimized_register_home_custody};

impl sealed::Sealed for StagedOptimizedRegisterHomes {}

impl AllocationSource for StagedOptimizedRegisterHomes {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_register_home_custody(
            self.legality_stage(),
            self.homes(),
            self.post_allocation_manifest(),
        )
        .map_err(AllocationReplayError::RegisterHomes)?;
        if evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        Ok(self.project_allocation())
    }
}

impl ProjectAllocation for StagedOptimizedRegisterHomes {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: self.selected().plan(),
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
            evidence: AllocationEvidence::RegisterHomes(self.custody().to_owned()),
        }
    }
}
