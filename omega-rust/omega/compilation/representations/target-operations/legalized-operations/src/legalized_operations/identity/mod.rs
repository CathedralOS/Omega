//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

use crate::legalized_operations::{LegalizedOperationPlan, LegalizedOperationPlanIdentity};
mod boundary_invocations;
mod calling;
pub use boundary_invocations::encode_boundary_execution_identity;
mod canonical;
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
pub use calling::{
    encode_call_plan, encode_callback_materialization, encode_eightbyte_class,
    encode_indirect_pointer, encode_native_place, encode_placement, encode_shape,
};
pub use normalized_foreign::{encode_call_binding, encode_machine_function_identity, encode_native_callback_argument};
pub use read_byte::encode_hosted_read_byte_identity;
pub use scalar::{encode_integer, encode_register, encode_scalar_type};
pub use structural::encode_provider_execution;
