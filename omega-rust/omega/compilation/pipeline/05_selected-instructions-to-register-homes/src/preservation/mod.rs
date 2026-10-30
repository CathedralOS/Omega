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

use crate::AllocationOutput;
use optimization_core::OptimizationWorkBudget;
pub use register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity,
};

pub fn stage_allocated_callee_saved_requirements(
    current: &AllocationOutput<'_>,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let plan = compute::derive(current, policy, budget)?;
    let receipt = custody::seal(&plan);
    Ok(ValidatedAllocatedCalleeSavedRequirements { plan, receipt })
}

/// The requirement plan's published identity, and nothing else: the plan beside
/// it owns every fact this boundary hands downstream, so the receipt restates
/// none of them. Downstream stages reproduce `identity()`; whole-plan rows are
/// read from [`AllocatedCalleeSavedRequirementPlan`] itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatedCalleeSavedRequirementReceipt {
    pub(in crate::preservation) identity: AllocatedCalleeSavedRequirementIdentity,
}

impl AllocatedCalleeSavedRequirementReceipt {
    receipt_accessor_rows!(AllocatedCalleeSavedRequirementReceipt {
        identity: AllocatedCalleeSavedRequirementIdentity,
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
