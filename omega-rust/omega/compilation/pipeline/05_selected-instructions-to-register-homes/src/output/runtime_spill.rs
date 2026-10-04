use super::{AllocationAdmissionError, AllocationEvidence, AllocationOutput};
use crate::assignment::runtime_spill::RuntimeSpillAllocation;

impl RuntimeSpillAllocation {
    /// Project the recovery's final rewrite and its facts into the current
    /// view. A recovery always records at least one step; one that recorded
    /// none has nothing to publish.
    pub(super) fn project_allocation(
        &self,
    ) -> Result<AllocationOutput<'_>, AllocationAdmissionError> {
        let rewrite = &self
            .steps
            .last()
            .ok_or(AllocationAdmissionError::EmptyRuntimeSpillRecovery)?
            .rewrite;
        Ok(AllocationOutput {
            program: register_homes::AllocatedProgramRef {
                selected: rewrite.transformed(),
                homes: self.homes.plan(),
            },
            selected: rewrite.selected(),
            liveness: &self.facts.liveness,
            ranges: &self.facts.ranges,
            legality: &self.facts.legality,
            homes: &self.homes,
            manifest: &self.manifest,
            environment: self.source.register_environment(),
            target_input: self.source.optimized_target_owner(),
            selections: self.source.selections(),
            budget: self.source.budget_per_pass(),
            evidence: AllocationEvidence::RuntimeSpill(self.manifest.record().identity),
        })
    }
}
