use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    ProjectAllocation, sealed,
};
use crate::SelectedProgramRef;
use crate::{
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
};

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
        let reanalysis = self.reanalysis_stage();
        let copies = reanalysis.transformation_stage();
        let selected = copies
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: &copies.copies().plan().transformed,
                homes: self.homes().plan(),
            },
            selected: SelectedProgramRef::new(copies.copies()),
            liveness: reanalysis.liveness(),
            ranges: reanalysis.ranges(),
            legality: reanalysis.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: selected.register_environment(),
            target_input: selected.optimized_target_owner(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
            evidence: AllocationEvidence::FixedViewCopies(self.custody().to_owned()),
        }
    }
}
