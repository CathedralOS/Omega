//! Immutable current allocation plus admitted facts.

use std::sync::Arc;

use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedProgram;
use selected_instructions_to_selected_instructions::{
    OwnedSelectedProgram, SelectedProgramRef, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedSelectedAnalysis,
};

use super::{AllocationEvidence, AllocationOutput};

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
    pub(super) fn capture(source: &AllocationOutput<'_>) -> Self {
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
}
