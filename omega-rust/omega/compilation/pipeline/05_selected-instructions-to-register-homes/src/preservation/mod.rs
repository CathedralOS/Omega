//! Optimizer module role: executable entrance. Allocation-visible ABI preservation requirements.
//!
//! This current-allocation boundary reports selected writes intersecting the exact
//! ABI callee-saved roster, then admits them only after independent keyed
//! replay. It grants no save/restore, frame, unwind, or publication authority.

mod compute;
mod custody;
mod error;
mod identity;
mod replay;
mod validation;

pub use error::AllocatedCalleeSavedRequirementError;
pub(crate) use identity::allocated_callee_saved_requirement_identity;
pub use validation::validate_allocated_callee_saved_requirements;

use optimization_core::OptimizationWorkBudget;

use crate::{AllocationOutput, RegisterHomeIdentity};
use optimization_core::{OptimizationWorkUsage, PostAllocationOptimizationManifestIdentity};
pub use register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity,
};
use register_model::{
    FrameAbiPreservationConvention, PhysicalRegisterModelIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use target::NativeTarget;

pub fn stage_allocated_callee_saved_requirements(
    current: &AllocationOutput<'_>,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let plan = compute::derive(current, policy, budget)?;
    let receipt = custody::seal(&plan);
    Ok(ValidatedAllocatedCalleeSavedRequirements { plan, receipt })
}

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
    receipt_accessor_rows!(AllocatedCalleeSavedRequirementReceipt {
        identity: AllocatedCalleeSavedRequirementIdentity,
        selected: SelectedInstructionPlanIdentity,
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
        physical_register_model: PhysicalRegisterModelIdentity,
        target: NativeTarget,
        abi: FrameAbiPreservationConvention,
        policy: AllocatedCalleeSavedRequirementPolicy,
        usage: OptimizationWorkUsage,
        function_count: usize,
        modified_function_count: usize,
        modified_unit_count: usize,
        witness_count: usize,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatedCalleeSavedRequirements {
    pub(in crate::preservation) plan: AllocatedCalleeSavedRequirementPlan,
    pub(in crate::preservation) receipt: AllocatedCalleeSavedRequirementReceipt,
}

impl ValidatedAllocatedCalleeSavedRequirements {
    receipt_accessor_rows!(borrow ValidatedAllocatedCalleeSavedRequirements {
        plan: &AllocatedCalleeSavedRequirementPlan,
    });
    receipt_accessor_rows!(ref ValidatedAllocatedCalleeSavedRequirements {
        receipt: AllocatedCalleeSavedRequirementReceipt,
    });
}
