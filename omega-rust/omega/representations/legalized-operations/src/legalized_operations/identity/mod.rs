//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

mod calling;
mod canonical;
mod condition;
mod legacy;
mod plan;
mod projected_structural_call_return;
mod scalar;
mod scalar_graph;
mod scalar_leaf;
mod schema;
mod shared;
mod structural;
mod structural_types;

use schema::{IdentitySchema, identity};
use shared::*;

#[doc(hidden)]
pub use legacy::*;

pub fn legalized_operation_plan_identity(
    plan: &LegalizedOperationPlan,
) -> LegalizedOperationPlanIdentity {
    identity(plan, IdentitySchema::V26)
}
