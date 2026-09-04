use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource, sealed,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization,
    validate_optimized_active_resident_rematerialization,
};
use omega_regalloc::SelectedProgramRef;

impl sealed::Sealed for StagedOptimizedActiveResidentRematerialization {}

impl AllocationSource for StagedOptimizedActiveResidentRematerialization {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let evidence = validate_optimized_active_resident_rematerialization(self)
            .map_err(AllocationReplayError::ActiveResidentRematerialization)?;
        if evidence != self.custody() {
            return Err(AllocationReplayError::ReceiptMismatch);
        }
        let selected = self
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        Ok(AllocationOutput {
            selected: SelectedProgramRef::new(self.rematerialization()),
            liveness: self.liveness(),
            ranges: self.ranges(),
            legality: self.legality(),
            homes: self.homes(),
            manifest: self.post_allocation_manifest(),
            environment: selected.register_environment(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
            evidence: AllocationEvidence::ActiveResidentRematerialization(evidence),
        })
    }
}
