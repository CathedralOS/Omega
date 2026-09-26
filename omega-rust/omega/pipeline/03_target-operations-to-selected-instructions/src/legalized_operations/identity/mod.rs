//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

use crate::legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
mod calling;
mod canonical;
mod dynamic_calls;
mod encoding;
mod normalized_foreign;
mod plan;
mod scalar;
mod scalar_graph;
mod structural;
mod structural_result;
mod structural_types;

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    canonical::identity(plan)
}
mod ordinary_calls;
mod read_byte;
pub use read_byte::encode_hosted_read_byte_identity;
