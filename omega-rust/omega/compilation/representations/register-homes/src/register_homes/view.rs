//! Borrowed access to the same allocated program, not another representation.

use crate::{
    AllocatedProgram, AllocationLegalityIdentity, AllocationLegalityPlan,
    PostAllocationOptimizationManifest, RegisterHomeIdentity, RegisterHomePlan,
};
use selected_instructions::{
    CurrentSelectedProgram, LiveRangeIdentity, LiveRangePlan, PreAllocationMachineEffectPlan,
    SelectedInstructionPlan,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct AllocatedProgramRef<'program> {
    pub selected: &'program SelectedInstructionPlan,
    pub homes: &'program RegisterHomePlan,
}

/// The current allocated program and the facts the allocation stage admitted
/// with it, as the post-allocation machine stage reads them: each plan beside
/// the identity its producer computed for it.
#[derive(Debug, Clone, Copy)]
pub struct AllocatedProgramFacts<'program> {
    pub selected: CurrentSelectedProgram<'program>,
    pub effects: &'program Arc<PreAllocationMachineEffectPlan>,
    pub ranges: &'program LiveRangePlan,
    pub ranges_identity: LiveRangeIdentity,
    pub legality: &'program AllocationLegalityPlan,
    pub legality_identity: AllocationLegalityIdentity,
    pub homes: &'program RegisterHomePlan,
    pub homes_identity: RegisterHomeIdentity,
    pub manifest: &'program PostAllocationOptimizationManifest,
}

impl AllocatedProgram {
    pub fn as_ref(&self) -> AllocatedProgramRef<'_> {
        AllocatedProgramRef {
            selected: &self.selected,
            homes: &self.homes,
        }
    }
}
