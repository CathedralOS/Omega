mod derivation;
mod model;
mod operator_applications;
mod projection;

pub(crate) use derivation::derive_physical_evidence;
pub use model::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, NativeByteSpan,
    NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection, NativePhysicalChild,
    NativePhysicalChildParts, NativePhysicalEvidence, NativePhysicalEvidenceParts,
    NativePhysicalOccurrence, OptimizedBoundaryOccurrence, OptimizedOperatorOccurrence,
    PhysicalChildParent, PhysicalRelocationDisposition,
    ValidatedOptimizedNativePhysicalEvidenceScope,
};
pub(crate) use projection::derive_validated_optimization_scope;
