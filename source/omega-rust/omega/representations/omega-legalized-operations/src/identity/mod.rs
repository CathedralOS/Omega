//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

mod calling;
mod canonical;
mod plan;
mod projected_structural_call_return;
mod scalar;
mod shared;
mod structural;
mod structural_types;

use shared::*;

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    canonical::identity(plan, b"omega.terminal-legalized-operations.v12\0", true)
}

#[doc(hidden)]
pub fn legalized_operation_plan_identity_v9_legacy(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    canonical::identity(plan, b"omega.terminal-legalized-operations.v9\0", false)
}
