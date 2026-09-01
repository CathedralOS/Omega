mod derivation;
mod model;
mod operator_applications;

pub(crate) use derivation::derive_physical_evidence;
pub use model::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, NativeByteSpan,
    NativeCompilerBuiltinCatalogIdentity, NativeIdentityOptimizationProjection,
    NativePhysicalChild, NativePhysicalChildParts, NativePhysicalEvidence,
    NativePhysicalEvidenceParts, NativePhysicalOccurrence, OptimizedBoundaryOccurrence,
    OptimizedOperatorOccurrence, PhysicalChildParent, PhysicalRelocationDisposition,
};
