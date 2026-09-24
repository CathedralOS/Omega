use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    ProjectAllocation, sealed,
};
use crate::{
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
};
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl sealed::Sealed for StagedOptimizedRegisterHomesAfterFixedViewCopies {}

impl AllocationSource for StagedOptimizedRegisterHomesAfterFixedViewCopies {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_register_home_after_fixed_view_copy_custody(
            self.reanalysis_stage(),
            self.homes(),
            self.post_allocation_manifest(),
        )
        .map_err(AllocationReplayError::FixedViewCopies)?;
        if evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        Ok(self.project_allocation())
    }
}

impl ProjectAllocation for StagedOptimizedRegisterHomesAfterFixedViewCopies {
    fn project_allocation(&self) -> AllocationOutput<'_> {
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: &self.selected().plan().transformed,
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
            evidence: AllocationEvidence::FixedViewCopies(self.custody().to_owned()),
        }
    }
}
