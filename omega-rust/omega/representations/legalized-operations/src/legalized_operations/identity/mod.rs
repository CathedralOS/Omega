//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

mod calling;
mod canonical;
mod plan;
mod projected_structural_call_return;
mod scalar;
mod scalar_graph;
mod shared;
mod structural;
mod structural_types;

use shared::*;

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    canonical::identity(plan)
}
mod ordinary_calls;
mod read_byte;
pub use read_byte::encode_hosted_read_byte_identity;
