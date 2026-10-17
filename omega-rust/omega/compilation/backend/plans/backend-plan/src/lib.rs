//! Callback and target-provider records consumed by realization.
//!
//! Produced placements retain the selected ABI and portable contract identity.
//! Callback artifacts retain Terminal bodies, and occurrence projection
//! binds them to the caller's portable applications. Native consumers receive
//! that body and ABI join, not a second callback scheduling pipeline.
//! Target-provider proposals retain whole selections beside the same artifact;
//! receiving admission closes every checked row without supplying host authority.

mod callback_artifacts;
mod callback_placements;
mod direct_foreign_leaves;
pub use direct_foreign_leaves::DirectForeignLeaf;
mod target_providers;
pub use callback_artifacts::{
    NativeCallbackThunkSettlement, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, native_callback_thunks, validate_callback_thunk_joins,
};
pub use callback_placements::{
    BoundCallbackPrivateMaterialization, NativeCallbackEntryParameter, NativeCallbackPlacement,
};
pub use target_providers::{
    TerminalTargetProviderPlan, TerminalTargetProviderProposal, TerminalTargetProviderSelection,
};
