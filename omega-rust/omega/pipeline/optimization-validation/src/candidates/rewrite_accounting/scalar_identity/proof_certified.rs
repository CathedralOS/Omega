use super::*;

pub(crate) fn reconstruct_proof_certified_scalar_identity_accounting(
    function: &PsiOptimizationFunction,
    patch: ProofCertifiedScalarIdentityRewrite,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    super::common::reconstruct_scalar_identity_accounting(
        function,
        patch.location,
        patch.source_operation,
        patch.result,
        ScalarType::Integer(patch.scalar_type),
    )
}
