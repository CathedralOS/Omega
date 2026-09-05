use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity,
    PreservationStorageCatalogIdentity, PreservationStorageGroupId, RegisterUnitId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use register_homes::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedUnitRequirement,
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
