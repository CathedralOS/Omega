//! Optimizer module role: executable entrance. Non-authoritative callee-save storage.
//!
//! This boundary maps validated modified ABI-preserved units through the
//! target-owned storage catalog, assigns canonical abstract area-relative
//! slots, and admits the result only after independent keyed replay. It grants
//! no frame, save/restore instruction, memory, unwind, or publication authority.

mod compute;
mod custody;
mod error;
mod identity;
mod replay;
mod validation;

pub use error::*;
pub use identity::non_authoritative_callee_save_storage_identity;
pub use validation::validate_non_authoritative_callee_save_storage;

use optimization_core::OptimizationWorkBudget;

pub use machine_code::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity,
    NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStoragePolicy,
};
use optimization_core::OptimizationWorkUsage;
use register_environment::{FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment};
use register_model::{
    PhysicalRegisterModelIdentity, PreservationStorageCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedUnitRequirement, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements, encode_callee_saved_modification_witness_identity,
};
use target::NativeTarget;

pub fn stage_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let plan = compute::derive(source, environment, policy, budget)?;
    validate_non_authoritative_callee_save_storage(source, environment, plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStorageReceipt {
    pub(in crate::frame_layout::save_storage) identity: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(in crate::frame_layout::save_storage) callee_saved_requirements:
        AllocatedCalleeSavedRequirementIdentity,
    pub(in crate::frame_layout::save_storage) register_environment:
        TargetRegisterEnvironmentIdentity,
    pub(in crate::frame_layout::save_storage) physical_register_model:
        PhysicalRegisterModelIdentity,
    pub(in crate::frame_layout::save_storage) preservation_storage_catalog:
        PreservationStorageCatalogIdentity,
    pub(in crate::frame_layout::save_storage) target: NativeTarget,
    pub(in crate::frame_layout::save_storage) abi: FrameAbiPreservationConvention,
    pub(in crate::frame_layout::save_storage) policy: NonAuthoritativeCalleeSaveStoragePolicy,
    pub(in crate::frame_layout::save_storage) usage: OptimizationWorkUsage,
    pub(in crate::frame_layout::save_storage) function_count: usize,
    pub(in crate::frame_layout::save_storage) modified_function_count: usize,
    pub(in crate::frame_layout::save_storage) slot_count: usize,
    pub(in crate::frame_layout::save_storage) modified_unit_count: usize,
    pub(in crate::frame_layout::save_storage) witness_count: usize,
    pub(in crate::frame_layout::save_storage) max_abstract_area_bytes: u64,
    pub(in crate::frame_layout::save_storage) max_abstract_area_alignment: u64,
}

impl NonAuthoritativeCalleeSaveStorageReceipt {
    pub const fn identity(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.identity
    }
    pub const fn callee_saved_requirements(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.callee_saved_requirements
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn physical_register_model(self) -> PhysicalRegisterModelIdentity {
        self.physical_register_model
    }
    pub const fn preservation_storage_catalog(self) -> PreservationStorageCatalogIdentity {
        self.preservation_storage_catalog
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn abi(self) -> FrameAbiPreservationConvention {
        self.abi
    }
    pub const fn policy(self) -> NonAuthoritativeCalleeSaveStoragePolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn modified_function_count(self) -> usize {
        self.modified_function_count
    }
    pub const fn slot_count(self) -> usize {
        self.slot_count
    }
    pub const fn modified_unit_count(self) -> usize {
        self.modified_unit_count
    }
    pub const fn witness_count(self) -> usize {
        self.witness_count
    }
    pub const fn max_abstract_area_bytes(self) -> u64 {
        self.max_abstract_area_bytes
    }
    pub const fn max_abstract_area_alignment(self) -> u64 {
        self.max_abstract_area_alignment
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNonAuthoritativeCalleeSaveStorage {
    pub(in crate::frame_layout::save_storage) plan: NonAuthoritativeCalleeSaveStoragePlan,
    pub(in crate::frame_layout::save_storage) receipt: NonAuthoritativeCalleeSaveStorageReceipt,
}

impl ValidatedNonAuthoritativeCalleeSaveStorage {
    pub const fn plan(&self) -> &NonAuthoritativeCalleeSaveStoragePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> NonAuthoritativeCalleeSaveStorageReceipt {
        self.receipt
    }
}
