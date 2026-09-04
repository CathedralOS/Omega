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
use omega_regalloc::SelectedProgramRef;

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
        let folds = self.fold_stage();
        let selected = folds
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let step = folds.final_step();
        AllocationOutput {
            selected: SelectedProgramRef::new(step.fold()),
            liveness: step.liveness(),
            ranges: step.ranges(),
            legality: step.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: selected.register_environment(),
            target_input: selected.optimized_target(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
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
        let run = self.selected_lowering_run();
        let source = run.source_legality_stage();
        let selected = source.live_range_stage().liveness_stage().selected_stage();
        let (program, liveness, ranges, legality) = match run.steps().last() {
            Some(step) => (
                SelectedProgramRef::new(step.fold()),
                step.liveness(),
                step.ranges(),
                step.legality(),
            ),
            None => (
                SelectedProgramRef::new(selected.selected()),
                source.live_range_stage().liveness_stage().liveness(),
                source.live_range_stage().ranges(),
                source.legality(),
            ),
        };
        AllocationOutput {
            selected: program,
            liveness,
            ranges,
            legality,
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: selected.register_environment(),
            target_input: selected.optimized_target(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
            evidence: AllocationEvidence::SelectedLowering(self.custody().to_owned()),
        }
    }
}
