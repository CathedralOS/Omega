//! A placeholder machine plan for unit tests whose records must point at a
//! plan but never read its rows. Its roots are fixed placeholder bytes, so it
//! matches no plan a compilation produces.
use std::sync::Arc;

use register_homes::{
    AllocatorAvailabilityIdentity, PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
    RegisterHomePlan,
};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::MachineEffectCatalogIdentity;
use target::NativeTarget;

use crate::{MachineAlternativeChoiceRule, PostAllocationMachinePlan};

pub fn placeholder_plan() -> Arc<PostAllocationMachinePlan> {
    let selected = selected_instructions::test_support::placeholder_plan(1);
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes([7; 32]);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes([5; 32]);
    let homes = Arc::new(RegisterHomePlan {
        selected: Arc::clone(&selected),
        register_environment,
        allocator_availability,
        functions: Vec::new(),
    });
    let manifest = Arc::new(PostAllocationOptimizationManifest {
        stage: PostAllocationManifestStage::RegisterHomes,
        optimization_unit: optimization_core::OptimizationUnitIdentity::from_bytes([6; 32]),
        selection: optimization_core::OptimizationSelectionIdentity::from_bytes([6; 32]),
        budget_per_pass: optimization_core::OptimizationWorkBudget::new(1, 1, 1, 1, 1)
            .expect("placeholder budget is valid"),
        target: NativeTarget::linux_x64(),
        selected: Arc::clone(&selected),
        selected_lowering_output: None,
        pre_allocation_output: None,
        selected_transformations: Vec::new(),
        register_environment,
        allocator_availability,
        homes: Arc::clone(&homes),
        spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
        frame: PostAllocationUnavailableData::Unavailable,
        emission: PostAllocationUnavailableData::Unavailable,
        publication: PostAllocationUnavailableData::Unavailable,
        statistics: PostAllocationStatistics::default(),
    });
    Arc::new(PostAllocationMachinePlan {
        selected,
        homes,
        post_allocation_manifest: manifest,
        target: NativeTarget::linux_x64(),
        register_environment,
        physical_register_model: PhysicalRegisterModelIdentity::from_bytes([8; 32]),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([9; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([10; 32]),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: Vec::new(),
    })
}
