mod derivation;
mod model;

pub(crate) use derivation::derive_physical_evidence;
pub use model::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, NativeByteSpan,
    NativeCompilerBuiltinCatalogIdentity, NativeIdentityOptimizationProjection,
    NativePhysicalChild, NativePhysicalChildParts, NativePhysicalEvidence,
    NativePhysicalEvidenceParts, OptimizedBoundaryOccurrence, PhysicalChildParent,
    PhysicalRelocationDisposition,
};
