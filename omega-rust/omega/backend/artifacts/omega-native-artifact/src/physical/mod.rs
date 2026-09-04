mod derivation;
mod model;
mod operator_applications;
mod projection;
mod selected_lowering;

pub(crate) use derivation::derive_physical_evidence;
pub use model::{
    BoundaryTraitSettlement, BoundaryTraitSettlementParts, BoundaryTraitSettlementRole,
    NativeByteSpan, NativeCompilerBuiltinCatalogIdentity, NativeOptimizationProjection,
    NativePhysicalChild, NativePhysicalChildParts, NativePhysicalEvidence,
    NativePhysicalEvidenceParts, NativePhysicalOccurrence, NormalizedForeignCallRelocation,
    NormalizedForeignCallbackRelocation, NormalizedForeignCallbackRelocations,
    OptimizedBoundaryOccurrence, OptimizedOperatorOccurrence, PhysicalChildParent,
    PhysicalRelocationDisposition, ValidatedOptimizedNativePhysicalEvidenceScope,
};
pub(crate) use projection::{
    derive_validated_optimization_scope, derive_validated_selected_lowering_optimization_scope,
};
pub use selected_lowering::SelectedLoweringNativePublicationInput;
