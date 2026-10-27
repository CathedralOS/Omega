use super::{
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementReceipt,
    allocated_callee_saved_requirement_identity,
};

/// Seal the plan as the identity its consumers reproduce. The plan is published
/// beside this receipt and owns every row the identity covers, so the receipt
/// carries no second copy of them.
pub(super) fn seal(
    plan: &AllocatedCalleeSavedRequirementPlan,
) -> AllocatedCalleeSavedRequirementReceipt {
    AllocatedCalleeSavedRequirementReceipt {
        identity: allocated_callee_saved_requirement_identity(plan),
    }
}
