//! The allocator's completed body, without allocation procedures or replay code.
//! Plans stay shared with optional replay evidence; machine lowering borrows the facts.
#![forbid(unsafe_code)]
use register_homes::{
    AllocatedCalleeSavedRequirements, AllocatedProgramFacts, AllocationLegalityPlan,
    AllocationPublicationFacts, RegisterHomePlan,
};
use selected_instructions::{
    CurrentSelectedProgram, LiveRangePlan, PreAllocationMachineEffectPlan, SelectedProgram,
};
use std::sync::Arc;

/// Records assembled by allocation after its required admission checks.
/// Possessing these records alone does not issue an allocated body.
#[derive(Debug, Clone)]
pub struct AllocationParts {
    pub body: Arc<physical_program::NativeBodyFacts>,
    pub selected: SelectedProgram,
    pub publication: Arc<AllocationPublicationFacts>,
    pub preservation: AllocatedCalleeSavedRequirements,
    pub environment: register_environment::ValidatedTargetRegisterEnvironment,
    pub effects: Arc<PreAllocationMachineEffectPlan>,
    pub ranges: Arc<LiveRangePlan>,
    pub legality: Arc<AllocationLegalityPlan>,
    pub homes: Arc<RegisterHomePlan>,
}

/// Only register allocation may issue this product. Later stages cannot issue it from bare facts.
/// ```compile_fail,E0277
/// fn bypass(parts: allocated_body::AllocationParts) -> allocated_body::AllocatedBody {
///     parts.into()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AllocatedBody {
    parts: AllocationParts,
}
impl AllocatedBody {
    pub fn new(_issuer: allocated_body_issuer::AllocationIssuer, parts: AllocationParts) -> Self {
        Self { parts }
    }
    pub fn body_facts(&self) -> &Arc<physical_program::NativeBodyFacts> {
        &self.parts.body
    }
    pub fn facts(&self) -> AllocatedProgramFacts<'_> {
        let parts = &self.parts;
        AllocatedProgramFacts {
            publication: &parts.publication,
            preservation: &parts.preservation,
            environment: &parts.environment,
            selected: CurrentSelectedProgram {
                plan: &parts.selected.plan,
                optimization_unit: parts.selected.optimization_unit,
                work_schedule: parts.selected.work_schedule,
            },
            effects: &parts.effects,
            ranges: &parts.ranges,
            legality: &parts.legality,
            homes: &parts.homes,
        }
    }
}
