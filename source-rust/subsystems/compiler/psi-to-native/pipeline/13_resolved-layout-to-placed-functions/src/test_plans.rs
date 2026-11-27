//! Placeholder frame and layout plans for unit tests that need a plan to point
//! at but never read its rows. Both point at the same placeholder machine plan.
use std::sync::Arc;

use machine_code::{ResolvedMachineLayout, TargetFrameLayoutPlan};
use target::NativeTarget;

pub(crate) fn frame_plan() -> Arc<TargetFrameLayoutPlan> {
    Arc::new(TargetFrameLayoutPlan {
        post_allocation_machine: post_allocation_machine::test_support::placeholder_plan(),
        callee_saved_requirements:
            register_homes::AllocatedCalleeSavedRequirementIdentity::from_bytes([12; 32]),
        callee_save_storage: machine_code::NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(
            [13; 32],
        ),
        register_environment: register_model::TargetRegisterEnvironmentIdentity::from_bytes(
            [4; 32],
        ),
        physical_register_model: register_model::PhysicalRegisterModelIdentity::from_bytes([5; 32]),
        target: NativeTarget::linux_x64(),
        abi: register_model::FrameAbiPreservationConvention::SystemVAMD64,
        policy: machine_code::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        functions: Vec::new(),
    })
}

pub(crate) fn resolved_layout() -> Arc<ResolvedMachineLayout> {
    Arc::new(ResolvedMachineLayout {
        machine: post_allocation_machine::test_support::placeholder_plan(),
        pre_layout: machine_code::SelectedFormEncodingIdentity::from_bytes([6; 32]),
        target: NativeTarget::linux_x64(),
        policy: machine_code::SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        functions: Vec::new(),
    })
}
