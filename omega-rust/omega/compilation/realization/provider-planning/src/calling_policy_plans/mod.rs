//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` models and collects the signatures,
//! `plan_computation.rs` evaluates the policy into plans,
//! `policy_implementations.rs` selects the exact named map's plan row,
//! `callback_bindings.rs` binds and closes callbacks; backend `layout` constructs
//! their native value graphs and `build_time_decoding.rs` decodes build-time
//! values. `callback_layout_catalog` associates backend-owned callback layouts
//! with exact boundary parameters and demands; `native_parameters` and
//! `opaque_representations` retain their respective signature associations.

mod boundary_signatures;
mod build_time_decoding;
mod callback_bindings;
mod callback_layout_catalog;
mod native_parameters;
mod opaque_representations;
mod plan_computation;
mod policy_implementations;

pub use boundary_signatures::{
    BoundaryCallbackBinder, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
pub(crate) use boundary_signatures::{BoundaryDirectCallbackParameter, BoundaryNativeParameter};
pub use callback_bindings::{
    BoundaryCallingPlanRealization, SelectedCallbackPlacement,
    close_outbound_callback_materializations, validate_nominal_callback_placement_bindings,
};
pub use callback_layout_catalog::BoundaryCallbackLayoutEntry;
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryOpaqueRepresentationUse,
};
pub use plan_computation::compute_boundary_calling_plans;
pub(crate) use plan_computation::evaluate_default_calling_application;
