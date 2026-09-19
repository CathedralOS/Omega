#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.
//!
//! Start at `native_artifact.rs` for artifact construction and replay, or
//! `callable_entry.rs` for validated callable-entry staging.

mod callable_entry;
mod native_artifact;
mod physical;

pub use callable_entry::*;
pub use image_emission::BoundaryExecutionRecord;
pub use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts,
    DynamicElfNativeArtifactIdentity, DynamicElfNativeArtifactParts, NativeArtifact,
    NativeArtifactEmissionParts, NativeArtifactIdentity, NativeArtifactParts,
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};
pub use physical::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, BoundaryTraitSettlementRole,
    NativeByteSpan, NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection,
    NativePhysicalChild, NativePhysicalChildParts, NativePhysicalEvidence,
    NativePhysicalEvidenceGap, NativePhysicalEvidenceGapSubject, NativePhysicalEvidenceParts,
    NativePhysicalOccurrence, NormalizedForeignCallRelocation, NormalizedForeignCallbackRelocation,
    NormalizedForeignCallbackRelocations, OptimizedBoundaryOccurrence, OptimizedOperatorOccurrence,
    PhysicalChildParent, PhysicalRelocationDisposition,
    ValidatedOptimizedNativePhysicalEvidenceScope,
};
