use crate::RegisterHomeIdentity;
use optimization_core::{OptimizationWorkUsage, PostAllocationOptimizationManifestIdentity};
use register_model::{PhysicalRegisterModelIdentity, TargetRegisterEnvironmentIdentity};
use selected_instructions::SelectedInstructionPlanIdentity;
use target::NativeTarget;

use register_model::FrameAbiPreservationConvention;

pub use register_homes::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedUnitRequirement, CalleeSavedModificationWitness,
    FunctionAllocatedCalleeSavedRequirements,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementReceipt {
    pub(in crate::preservation) identity: AllocatedCalleeSavedRequirementIdentity,
    pub(in crate::preservation) selected: SelectedInstructionPlanIdentity,
    pub(in crate::preservation) homes: RegisterHomeIdentity,
    pub(in crate::preservation) post_allocation_manifest:
        PostAllocationOptimizationManifestIdentity,
    pub(in crate::preservation) register_environment: TargetRegisterEnvironmentIdentity,
    pub(in crate::preservation) physical_register_model: PhysicalRegisterModelIdentity,
    pub(in crate::preservation) target: NativeTarget,
    pub(in crate::preservation) abi: FrameAbiPreservationConvention,
    pub(in crate::preservation) policy: AllocatedCalleeSavedRequirementPolicy,
    pub(in crate::preservation) usage: OptimizationWorkUsage,
    pub(in crate::preservation) function_count: usize,
    pub(in crate::preservation) modified_function_count: usize,
    pub(in crate::preservation) modified_unit_count: usize,
    pub(in crate::preservation) witness_count: usize,
}

impl AllocatedCalleeSavedRequirementReceipt {
    pub const fn identity(self) -> AllocatedCalleeSavedRequirementIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn physical_register_model(self) -> PhysicalRegisterModelIdentity {
        self.physical_register_model
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn abi(self) -> FrameAbiPreservationConvention {
        self.abi
    }
    pub const fn policy(self) -> AllocatedCalleeSavedRequirementPolicy {
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
    pub const fn modified_unit_count(self) -> usize {
        self.modified_unit_count
    }
    pub const fn witness_count(self) -> usize {
        self.witness_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatedCalleeSavedRequirements {
    pub(in crate::preservation) plan: AllocatedCalleeSavedRequirementPlan,
    pub(in crate::preservation) receipt: AllocatedCalleeSavedRequirementReceipt,
}

impl ValidatedAllocatedCalleeSavedRequirements {
    pub const fn plan(&self) -> &AllocatedCalleeSavedRequirementPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AllocatedCalleeSavedRequirementReceipt {
        self.receipt
    }
}
