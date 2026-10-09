//! Callback ABI and body records consumed by native realization.
//!
//! Produced placements retain the selected ABI and portable contract identity.
//! Callback artifacts retain Terminal bodies, and occurrence projection
//! binds them to the caller's portable applications. Native consumers receive
//! that body and ABI join, not a second callback scheduling pipeline.

mod callback_artifacts;
mod callback_placements;
pub use callback_artifacts::{
    NativeCallbackThunkSettlement, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, native_callback_thunks, validate_callback_thunk_joins,
};
pub use callback_placements::{BoundCallbackPrivateMaterialization, NativeCallbackPlacement};
