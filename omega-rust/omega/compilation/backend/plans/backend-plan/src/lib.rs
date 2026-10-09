//! Callback preparation records at the source-to-Terminal handoff.
//!
//! Source-side placements retain the selected ABI and resource evidence.
//! Produced callback artifacts retain Terminal bodies, and occurrence projection
//! binds them to the caller's portable applications. Native consumers receive
//! that body and ABI join, not a second callback scheduling pipeline.

mod callback_artifacts;
mod callback_placements;
pub use callback_artifacts::{
    NativeCallbackThunkSettlement, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, native_callback_thunks, validate_callback_thunk_joins,
};
pub use callback_placements::{
    BoundCallbackPrivateMaterialization, BoundNominalCallbackPlacement,
    validate_bound_nominal_callback_placement,
};
