#![forbid(unsafe_code)]

//! Shared composition from canonical Terminal Psi to a replayed native artifact.
//!
//! This crate is named for its exact input and output. Its entrances settle
//! source-entry custody, coordinate ordinary or explicitly selected native
//! realization, and own the ProgramStorage semantic-wrapper join. It does not
//! own source compilation, component policy, installation, or publication.

mod entry_settlement;
mod optimized_semantic_wrapper_encoding;
mod optimized_semantic_wrapper_object;
mod realization;

pub use entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use omega_native_artifact::{
    NativeArtifact, NativeArtifactParts, NativeProviderExecution, NativeSelectedProviderPlan,
    RankedNativeFuelArtifact, RankedNativeFuelArtifactParts,
};
pub use optimized_semantic_wrapper_encoding::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
pub use optimized_semantic_wrapper_object::*;
pub use realization::{
    CallbackCustodyNativeRealizationError, NativeBoundaryRealization, NativeProviderSettlement,
    NativeRealizationRequest, RealizedNativeArtifactWithCallbackCustody, SettledNativeArtifact,
    realize_native_artifact, realize_native_artifact_with_callback_custody,
    realize_program_entry_native_artifact,
};

#[cfg(test)]
mod tests;
