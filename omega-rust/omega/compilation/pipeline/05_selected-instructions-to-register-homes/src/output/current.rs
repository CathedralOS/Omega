//! Immutable current allocation plus admitted facts.

use std::sync::Arc;

use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::{AllocatedProgram, AllocatedProgramFacts};
use selected_instructions::PreAllocationMachineEffectPlan;
use selected_instructions_to_selected_instructions::{
    MachineEffectStageError, OwnedSelectedProgram, SelectedProgramRef, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedSelectedAnalysis, analyze_machine_effects,
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
    /// Machine effects of the current program, analyzed once here so the
    /// post-allocation stage reads them instead of running the analysis.
    effects: Arc<PreAllocationMachineEffectPlan>,
    evidence: AllocationEvidence,
    // Prior semantic/provider input is retained for proof joins, not instruction lookup.
    target_input: Arc<ValidatedOptimizedTargetOperations>,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
}

impl CurrentAllocation {
    pub(super) fn capture(source: &AllocationOutput<'_>) -> Result<Self, MachineEffectStageError> {
        let effects = analyze_machine_effects(source.selected(), source.register_environment())?;
        let selected = OwnedSelectedProgram::retain(source.selected());
        Ok(Self {
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
            effects: effects.shared_plan(),
            evidence: source.evidence().clone(),
            target_input: Arc::clone(source.target_input),
            selections: source.selections().clone(),
            budget: source.budget_per_pass(),
        })
    }

    pub(super) fn facts(&self) -> AllocatedProgramFacts<'_> {
        AllocatedProgramFacts {
            selected: self.selected.current_program(),
            effects: &self.effects,
            ranges: self.ranges.plan(),
            ranges_identity: self.ranges.receipt().identity(),
            legality: self.legality.plan(),
            legality_identity: self.legality.receipt().identity(),
            homes: self.homes.plan(),
            homes_identity: self.homes.receipt().identity(),
            manifest: self.manifest.record(),
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
