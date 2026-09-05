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
        let ranges = self.legality_stage().live_range_stage();
        let liveness = ranges.liveness_stage();
        let selected = liveness.selected_stage();
        AllocationOutput {
            program: omega_register_homes::AllocatedProgramRef {
                selected: selected.selected().plan(),
                homes: self.homes().plan(),
            },
            selected: SelectedProgramRef::new(selected.selected()),
            liveness: liveness.liveness(),
            ranges: ranges.ranges(),
            legality: self.legality_stage().legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: selected.register_environment(),
            target_input: selected.optimized_target_owner(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
            evidence: AllocationEvidence::RegisterHomes(self.custody().to_owned()),
        }
    }
}
