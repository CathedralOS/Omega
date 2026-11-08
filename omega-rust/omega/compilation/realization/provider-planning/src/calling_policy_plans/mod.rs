//! Prepare source calling requests and activate retained Terminal policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.
//!
//! `boundary_signatures.rs` associates geometry with Source review coordinates.
//! `source_preparation.rs` captures original request/review correspondence and
//! target helper candidates once beside the shared Terminal publication.
//! `plan_computation.rs` publishes per-target plans from that owned preparation
//! and canonical storage, without reopening Source. `selected_providers.rs` applies the one
//! canonical invocation-scoped provider choice. `calling_program.rs` owns
//! finite invocation accounting and carries Build evaluation's executor.
//! `policy_invocation.rs` forms its source-free, single-use admitted request;
//! terminal-installation checks the canonical declaration and call closure.
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
mod policy_invocation;
mod policy_signature;
mod scalar_completion;
mod selected_application;
mod selected_providers;
mod signature_review;
mod source_preparation;
mod terminal_signature;
pub use build_time_decoding::decode_boundary_plan_result;
pub use calling_program::CallingProgram;
pub use policy_invocation::{
    CallingPolicyEvaluator, TerminalCallingPolicyAdmission, TerminalCallingPolicyInvocation,
    admit_terminal_calling_policy,
};
pub use policy_signature::CallingPolicySignature;
pub use policy_signature::{encode_calling_policy_signature, validate_calling_policy_result};
pub use scalar_completion::{CallingScalarScope, complete_calling_scalar_inputs};
pub use selected_application::component_calling_applications;
pub(crate) use selected_application::selected_calling_applications;
pub use selected_providers::CallingPolicyProviderInputs;
pub use source_preparation::CallingSourcePreparation;
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
