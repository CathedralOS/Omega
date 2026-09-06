mod derivation;
mod fragment_publication;
mod model;
mod operator_applications;
mod projection;

pub(crate) use derivation::derive_physical_evidence;
pub(crate) use fragment_publication::derive_scope as derive_fragment_publication_scope;
pub use model::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, BoundaryTraitSettlementRole,
    NativeByteSpan, NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection,
    NativePhysicalChild, NativePhysicalChildParts, NativePhysicalEvidence,
    NativePhysicalEvidenceParts, NativePhysicalOccurrence, NormalizedForeignCallRelocation,
    NormalizedForeignCallbackRelocation, NormalizedForeignCallbackRelocations,
    OptimizedBoundaryOccurrence, OptimizedOperatorOccurrence, PhysicalChildParent,
    PhysicalRelocationDisposition, ValidatedOptimizedNativePhysicalEvidenceScope,
};
pub(crate) use projection::derive_validated_optimization_scope;
