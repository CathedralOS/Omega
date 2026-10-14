//! Optimizer module role: stage group. Canonical identity construction for legalized-operation custody.

mod boundary_invocations;
mod calling;
pub use boundary_invocations::{encode_boundary_execution_identity, encode_placed_scalar_demand};
mod canonical;
mod encoding;
mod normalized_foreign;
mod plan;
mod scalar;
mod scalar_graph;
mod structural;
mod structural_result;
mod structural_types;

pub use canonical::identity as legalized_operation_plan_identity;
mod ordinary_calls;
mod read_byte;
pub use calling::{encode_call_plan, encode_placement, encode_shape};
pub use encoding::encode_fuel;
pub use normalized_foreign::{
    encode_call_binding, encode_locator, encode_machine_function_identity,
    encode_native_callback_argument,
};
pub use read_byte::encode_hosted_read_byte_identity;
pub use scalar::{encode_integer, encode_register, encode_scalar_type};
pub use structural::encode_provider_execution;

mod placed_bindings;
pub use placed_bindings::encode_placed_call_bindings;
