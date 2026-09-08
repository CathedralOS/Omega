use super::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource, sealed,
};
use crate::assignment::runtime_spill::{RuntimeSpillAllocation, replay};

impl sealed::Sealed for RuntimeSpillAllocation {}

impl AllocationSource for RuntimeSpillAllocation {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        replay::validate(self).map_err(AllocationReplayError::RuntimeSpill)?;
        let selected = self
            .source
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let rewrite = &self
            .steps
            .last()
            .ok_or(AllocationReplayError::ReceiptMismatch)?
            .rewrite;
        Ok(AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: rewrite.transformed(),
                homes: self.homes.plan(),
            },
            selected: crate::SelectedProgramRef::new(rewrite),
            liveness: &self.facts.liveness,
            ranges: &self.facts.ranges,
            legality: &self.facts.legality,
            homes: &self.homes,
            manifest: &self.manifest,
            environment: selected.register_environment(),
            target_input: selected.optimized_target_owner(),
            selections: selected.optimized_target().optimized().selections(),
            budget: selected.optimized_target().optimized().budget_per_pass(),
            evidence: AllocationEvidence::RuntimeSpill(self.manifest.record().identity),
        })
    }
}
