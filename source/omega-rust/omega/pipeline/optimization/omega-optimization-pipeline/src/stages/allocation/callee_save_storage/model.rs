use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    PhysicalRegisterModelIdentity, PreservationStorageCatalogIdentity, PreservationStorageGroupId,
    RegisterUnitId, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedUnitRequirement, FrameAbiPreservationConvention,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NonAuthoritativeCalleeSaveStorageIdentity([u8; 32]);

impl NonAuthoritativeCalleeSaveStorageIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NonAuthoritativeCalleeSaveStoragePolicy {
    CanonicalTargetPreservationGroupsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonAuthoritativeCalleeSaveSlotId(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveSlot {
    pub id: NonAuthoritativeCalleeSaveSlotId,
    pub storage_group: PreservationStorageGroupId,
    pub storage_view: RegisterViewId,
    pub preserved_units: Vec<RegisterUnitId>,
    pub modified_units: Vec<AllocatedCalleeSavedUnitRequirement>,
    /// Relative only to this artifact's abstract callee-save area origin.
    pub abstract_offset_bytes: u64,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionNonAuthoritativeCalleeSaveStorage {
    pub machine: MachineId,
    pub kind: AllocatedCalleeSavedFunctionKind,
    pub abstract_area_bytes: u64,
    pub abstract_area_alignment: u64,
    pub slots: Vec<NonAuthoritativeCalleeSaveSlot>,
}

/// Abstract preservation storage only. No field is a frame coordinate, stack
/// decision, executable access, instruction, unwind row, or publication fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStoragePlan {
    pub callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub preservation_storage_catalog: PreservationStorageCatalogIdentity,
    pub target: NativeTarget,
    pub abi: FrameAbiPreservationConvention,
    pub callee_saved_units: Vec<RegisterUnitId>,
    pub policy: NonAuthoritativeCalleeSaveStoragePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionNonAuthoritativeCalleeSaveStorage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStorageReceipt {
    pub(crate) identity: NonAuthoritativeCalleeSaveStorageIdentity,
    pub(crate) callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical_register_model: PhysicalRegisterModelIdentity,
    pub(crate) preservation_storage_catalog: PreservationStorageCatalogIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) abi: FrameAbiPreservationConvention,
    pub(crate) policy: NonAuthoritativeCalleeSaveStoragePolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) modified_function_count: usize,
    pub(crate) slot_count: usize,
    pub(crate) modified_unit_count: usize,
    pub(crate) witness_count: usize,
    pub(crate) max_abstract_area_bytes: u64,
    pub(crate) max_abstract_area_alignment: u64,
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
    pub(crate) plan: NonAuthoritativeCalleeSaveStoragePlan,
    pub(crate) receipt: NonAuthoritativeCalleeSaveStorageReceipt,
}

impl ValidatedNonAuthoritativeCalleeSaveStorage {
    pub const fn plan(&self) -> &NonAuthoritativeCalleeSaveStoragePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> NonAuthoritativeCalleeSaveStorageReceipt {
        self.receipt
    }
}
