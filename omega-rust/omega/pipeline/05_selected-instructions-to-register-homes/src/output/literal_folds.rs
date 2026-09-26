use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    ProjectAllocation, sealed,
};
use crate::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};
use selected_instructions_to_selected_instructions::SelectedProgramRef;

impl sealed::Sealed for StagedOptimizedRegisterHomesAfterLiteralFolds {}

impl AllocationSource for StagedOptimizedRegisterHomesAfterLiteralFolds {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_register_home_after_literal_fold_custody(self)
            .map_err(AllocationReplayError::LiteralFolds)?;
        if &evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        Ok(self.project_allocation())
    }
}

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

impl sealed::Sealed for StagedOptimizedRegisterHomesAfterSelectedLowering {}

impl AllocationSource for StagedOptimizedRegisterHomesAfterSelectedLowering {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_register_home_after_selected_lowering_custody(self)
            .map_err(AllocationReplayError::SelectedLowering)?;
        if &evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        Ok(self.project_allocation())
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
