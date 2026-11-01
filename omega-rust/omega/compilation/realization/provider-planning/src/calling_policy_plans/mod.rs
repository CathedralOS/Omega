//! Prepare source calling requests and activate retained Terminal policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` materializes Psi's prepared signatures,
//! `plan_computation.rs` retains admission and signature inputs, then evaluates
//! policies from the compiler's shared code catalog. `policy_code.rs` owns the
//! exact source-entry association and finite invocation accounting;
//! `terminal_execution.rs` runs the canonical code without source,
//! `policy_signature.rs` encodes canonical ABI input and validates the result,
//! Psi's service catalog retains the exact named map and inherited signatures,
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
mod policy_code;
mod policy_signature;
mod selected_application;
mod terminal_execution;
mod terminal_values;
pub use policy_code::CallingPolicyCode;
pub use policy_signature::CallingPolicySignature;
pub use selected_application::component_calling_applications;
pub(crate) use selected_application::selected_source_calling_application;
pub use terminal_execution::{TerminalCallingPolicyInvocation, evaluate_terminal_calling_policy};

pub use boundary_signatures::{
    BoundaryCallbackBinder, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
pub(crate) use boundary_signatures::{BoundaryDirectCallbackParameter, BoundaryNativeParameter};
pub use callback_bindings::{
    BoundaryCallingPlanRealization, close_outbound_callback_materializations,
};
pub use callback_layout_catalog::BoundaryCallbackLayoutEntry;
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryOpaqueRepresentationUse,
};
pub(crate) use plan_computation::evaluate_default_calling_application;
pub use plan_computation::{PreparedBoundaryCallingPlans, prepare_boundary_calling_plans};
