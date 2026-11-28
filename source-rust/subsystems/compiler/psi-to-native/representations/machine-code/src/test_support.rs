//! A placeholder frame plan for unit tests whose records must point at a frame
//! plan but never read its preservation rows. It shares the placeholder
//! machine plan's homes and manifest, so it matches no plan a compilation
//! produces.
use std::sync::Arc;

use register_homes::{AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity,
    PreservationStorageCatalogIdentity, TargetRegisterEnvironmentIdentity,
};
use target::NativeTarget;

use crate::{
    FunctionTargetFrameLayout, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, SelectedFormEncoding, SelectedFormEncodingHandle,
    TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
};

pub fn placeholder_frame_plan(functions: Vec<FunctionTargetFrameLayout>) -> TargetFrameLayoutPlan {
    let machine = post_allocation_machine::test_support::placeholder_plan();
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes([4; 32]);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes([5; 32]);
    let target = NativeTarget::linux_x64();
    let abi = FrameAbiPreservationConvention::SystemVAMD64;
    let budget = optimization_core::OptimizationWorkBudget::new(1, 1, 1, 1, 1)
        .expect("placeholder budget is valid");
    let requirements = Arc::new(AllocatedCalleeSavedRequirementPlan {
        selected: Arc::clone(&machine.selected),
        homes: Arc::clone(&machine.homes),
        post_allocation_manifest: Arc::clone(&machine.post_allocation_manifest),
        register_environment,
        physical_register_model,
        target,
        abi,
        callee_saved_units: Vec::new(),
        policy:
            AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
        usage: Default::default(),
        functions: Vec::new(),
    });
    let storage = Arc::new(NonAuthoritativeCalleeSaveStoragePlan {
        callee_saved_requirements: Arc::clone(&requirements),
        register_environment,
        physical_register_model,
        preservation_storage_catalog: PreservationStorageCatalogIdentity::from_bytes([13; 32]),
        target,
        abi,
        callee_saved_units: Vec::new(),
        policy: NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
        usage: Default::default(),
        functions: Vec::new(),
    });
    TargetFrameLayoutPlan {
        post_allocation_machine: machine,
        callee_saved_requirements: requirements,
        callee_save_storage: storage,
        register_environment,
        physical_register_model,
        target,
        abi,
        policy: TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        functions,
    }
}

/// A handle to one empty encoding of the placeholder machine plan, for
/// records that must name a pre-layout encoding but never read its rows. The
/// encoding lives for the whole test process, so every call names the same one.
pub fn placeholder_encoding() -> SelectedFormEncodingHandle {
    static ENCODING: std::sync::OnceLock<Arc<SelectedFormEncoding>> = std::sync::OnceLock::new();
    SelectedFormEncodingHandle::of(ENCODING.get_or_init(|| {
        Arc::new(SelectedFormEncoding {
            machine: post_allocation_machine::test_support::placeholder_plan(),
            rows: Vec::new(),
            encoded: Default::default(),
            frame: None,
            counts: Default::default(),
        })
    }))
}
