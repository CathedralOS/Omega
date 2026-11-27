//! A placeholder machine plan for unit tests whose records must point at a
//! plan but never read its rows. Its roots are fixed placeholder bytes, so it
//! matches no plan a compilation produces.
use std::sync::Arc;

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::RegisterHomeIdentity;
use register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::MachineEffectCatalogIdentity;
use target::NativeTarget;

use crate::{MachineAlternativeChoiceRule, PostAllocationMachinePlan};

pub fn placeholder_plan() -> Arc<PostAllocationMachinePlan> {
    Arc::new(PostAllocationMachinePlan {
        selected: selected_instructions::test_support::placeholder_plan(1),
        homes: RegisterHomeIdentity::from_bytes([5; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([6; 32]),
        target: NativeTarget::linux_x64(),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([7; 32]),
        physical_register_model: PhysicalRegisterModelIdentity::from_bytes([8; 32]),
        register_constraints: RegisterConstraintCatalogIdentity::from_bytes([9; 32]),
        machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([10; 32]),
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions: Vec::new(),
    })
}
