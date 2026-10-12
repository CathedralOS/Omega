//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` models and collects the signatures,
//! `applications.rs` joins authored and produced public interfaces;
//! `plan_computation.rs` extends one retained policy-plan inventory,
//! `policy_implementations.rs` selects the exact named map's plan row,
//! `callback_bindings.rs` binds and closes callbacks;
//! `native_applications.rs` publishes both completed policy-plan variants.
//! Backend `layout` constructs native value graphs; `build_time_decoding.rs`
//! decodes build-time values. `callback_layout_catalog` associates backend-owned callback layouts
//! with exact boundary parameters and demands; `native_parameters` and
//! `opaque_representations` retain their respective signature associations.

mod applications;
mod boundary_signatures;
mod build_time_decoding;
mod callback_bindings;
mod callback_layout_catalog;
mod native_applications;
mod native_parameters;
mod opaque_representations;
mod plan_computation;
mod policy_implementations;

pub(crate) use boundary_signatures::{
    call_signature_from_top_level_requirement, call_signature_from_typed,
    compatibility_call_signature,
};

pub(crate) use applications::calling_policy_owner;
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
pub use plan_computation::extend_boundary_calling_plans;
pub(crate) use plan_computation::{
    boundary_plan_application_identity, evaluate_compatibility_boundary_application,
    evaluate_compatibility_boundary_entry_plan,
};
