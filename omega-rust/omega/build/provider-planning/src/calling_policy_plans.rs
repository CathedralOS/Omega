//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` models and collects the signatures,
//! `plan_computation.rs` evaluates the policy into plans,
//! `callback_bindings.rs` binds and closes callbacks, `value_shapes.rs`
//! derives value shapes and `build_time_decoding.rs` decodes the build-time
//! values; `callback_layout_catalog`, `native_parameters` and
//! `opaque_representations` stay as they were.

mod boundary_signatures;
mod build_time_decoding;
mod callback_bindings;
mod callback_layout_catalog;
mod native_parameters;
mod opaque_representations;
mod plan_computation;
#[cfg(test)]
mod tests;
mod value_shapes;

pub use boundary_signatures::{
    BoundaryCallbackBinder, BoundaryDirectCallbackParameter, BoundaryNativeParameter,
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, BoundaryValueClass,
    BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
pub use callback_bindings::{
    BoundaryCallingPlanRealization, close_outbound_callback_materializations,
    validate_nominal_callback_placement_bindings,
};
pub use callback_layout_catalog::{BoundaryCallbackInlineField, BoundaryCallbackLayoutEntry};
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};
pub use plan_computation::{
    compute_boundary_calling_plans, evaluate_calling_policy_plan,
    evaluate_compatibility_boundary_entry_plan, materialized_boundary_signature_from_abi,
};
pub use value_shapes::selected_program_storage_source_extent_value_layout;
