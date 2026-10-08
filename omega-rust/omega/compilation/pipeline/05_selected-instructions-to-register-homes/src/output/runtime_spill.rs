use super::{AllocationEvidence, AllocationOutput};
use crate::assignment::runtime_spill::RuntimeSpillAllocation;

impl RuntimeSpillAllocation {
    /// Project the recovery's final rewrite and its facts into the current
    /// view.
    pub(super) fn project_allocation(&self) -> AllocationOutput<'_> {
        let rewrite = &self.last.rewrite;
        AllocationOutput {
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
        }
    }
}
