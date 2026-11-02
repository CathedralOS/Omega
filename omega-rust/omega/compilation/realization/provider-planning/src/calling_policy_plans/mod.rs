//! Prepare source calling requests and activate retained Terminal policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` associates geometry with Source review coordinates.
//! `plan_computation.rs` prepares authored admission and publishes default or
//! authored plans from canonical storage. `calling_program.rs` owns the
//! exact source-entry association and finite invocation accounting;
//! `terminal_execution.rs` runs the canonical code without source,
//! `policy_signature.rs` encodes canonical ABI input and validates the result,
//! `terminal_signature.rs` constructs that input from the canonical catalog;
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
mod calling_program;
mod native_parameters;
mod opaque_representations;
mod plan_computation;
mod policy_signature;
mod scalar_completion;
mod selected_application;
mod selected_providers;
mod signature_review;
mod terminal_execution;
mod terminal_signature;
mod terminal_values;
pub use calling_program::CallingProgram;
pub use policy_signature::CallingPolicySignature;
pub use selected_application::component_calling_applications;
pub(crate) use selected_application::selected_source_calling_applications;
pub use selected_providers::CallingPolicyProviderInputs;
pub use terminal_execution::{TerminalCallingPolicyInvocation, evaluate_terminal_calling_policy};
pub use terminal_signature::{CanonicalCallingSignature, CanonicalOpaqueUse};

pub use boundary_signatures::{
    BoundaryCallbackBinder, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
pub(crate) use boundary_signatures::{BoundaryDirectCallbackParameter, BoundaryNativeParameter};
pub use callback_bindings::BoundaryCallingPlanRealization;
pub use callback_layout_catalog::{
    BoundaryCallbackLayoutEntry, CallbackFieldReviewSource, CallbackLayoutReviewSource,
    CallbackRecordReviewSource, CallbackSlotReviewSource,
};
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryOpaqueRepresentationUse,
};
pub(crate) use plan_computation::evaluate_default_calling_applications;
pub use plan_computation::{PreparedBoundaryCallingPlans, prepare_boundary_calling_plans};
