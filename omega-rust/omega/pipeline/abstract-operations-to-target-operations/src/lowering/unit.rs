//! Operation-level call, storage, and scalar lowering shared by control graphs.
pub(super) mod boundary_call;
pub(super) mod dynamic;
mod projected_argument;
mod projected_result;
pub(super) mod scalar_call;
pub(super) mod scalar_definitions;
pub(super) mod structural_call;
mod structural_scalar;
pub(super) use structural_scalar::{
    lower_dynamic_argument_scalar_call, lower_dynamic_argument_unit_call, lower_field_store,
};
pub(super) mod write_only_primitive_store;
