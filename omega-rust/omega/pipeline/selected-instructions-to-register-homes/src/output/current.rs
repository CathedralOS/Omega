//! Immutable current allocation plus admitted facts, separate from replay inputs.

use std::sync::Arc;

use crate::{
    OwnedSelectedProgram, SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
    ValidatedSelectedAnalysis,
};
use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedProgram;

use super::{AllocationEvidence, AllocationOutput, AllocationReplayError};

/// This private admission capsule does not own allocation-stage ancestry.
/// Program data is representation-owned; facts, policy, and the retained
/// upstream proof input remain explicit alongside it.
#[derive(Debug)]
pub(super) struct CurrentAllocation {
    pub(super) program: AllocatedProgram,
    selected: OwnedSelectedProgram,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    environment: ValidatedTargetRegisterEnvironment,
    evidence: AllocationEvidence,
    // Prior semantic/provider input is retained for proof joins, not instruction lookup.
    target_input: Arc<ValidatedOptimizedTargetOperations>,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
}

impl CurrentAllocation {
    pub(super) fn from_replayed(source: &AllocationOutput<'_>) -> Self {
        let selected = OwnedSelectedProgram::retain(source.selected());
        Self {
            program: AllocatedProgram {
                selected: selected.shared_selected_plan(),
                homes: source.homes().shared_plan(),
            },
            selected,
            liveness: source.liveness().clone(),
            ranges: source.ranges().clone(),
            legality: source.legality().clone(),
            homes: source.homes().clone(),
            manifest: source.post_allocation_manifest().clone(),
            environment: source.register_environment().clone(),
            evidence: source.evidence().clone(),
            target_input: Arc::clone(source.target_input),
            selections: source.selections().clone(),
            budget: source.budget_per_pass(),
        }
    }

    pub(super) fn view(&self) -> AllocationOutput<'_> {
        AllocationOutput {
            program: self.program.as_ref(),
            selected: SelectedProgramRef::new(&self.selected),
            liveness: &self.liveness,
            ranges: &self.ranges,
            legality: &self.legality,
            homes: &self.homes,
            manifest: &self.manifest,
            environment: &self.environment,
            evidence: self.evidence.clone(),
            target_input: &self.target_input,
            selections: &self.selections,
            budget: self.budget,
        }
    }

    /// Independent replay must reconstruct every retained current fact. Pointer
    /// equality below checks the internal upstream-owner join only; it is not a
    /// persisted identity or a substitute for replay of that upstream subject.
    pub(super) fn validate_against(
        &self,
        replayed: &AllocationOutput<'_>,
    ) -> Result<(), AllocationReplayError> {
        if *self.program.selected != *replayed.selected_plan()
            || *self.program.homes != *replayed.program().homes
            || self.selected != OwnedSelectedProgram::retain(replayed.selected())
            || self.liveness != *replayed.liveness()
            || self.ranges != *replayed.ranges()
            || self.legality != *replayed.legality()
            || self.homes != *replayed.homes()
            || self.manifest != *replayed.post_allocation_manifest()
            || self.environment != *replayed.register_environment()
            || self.evidence != *replayed.evidence()
            || !Arc::ptr_eq(&self.target_input, replayed.target_input)
            || self.selections != *replayed.selections()
            || self.budget != replayed.budget_per_pass()
        {
            return Err(AllocationReplayError::CurrentProgramMismatch);
        }
        Ok(())
    }
}
