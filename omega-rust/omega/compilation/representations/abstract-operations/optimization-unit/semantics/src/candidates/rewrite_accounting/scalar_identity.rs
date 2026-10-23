//! Optimizer module role: stage group. Scalar-identity accounting families.
//!
//! Both obligation-free and proof-certified identities remove one scalar node
//! and substitute its live result. Their semantic admission remains separate;
//! only exact custody reconstruction is shared here, with the common
//! subexpression families running the same spine: each reconstruction names
//! its own patch, then runs the shared removed-result use-site accounting.

use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_removed_result_use_accounting;
use optimization_unit::{
    ProofCertifiedScalarIdentityRewrite, PsiOptimizationFunction, TotalScalarIdentityRewrite,
};
use semantic_vocabulary::{BlockId, ScalarType};

/// The shared field-projection each scalar-identity patch runs into the
/// removed-result use-site accounting spine.
macro_rules! scalar_identity_reconstruction {
    ($name:ident, $patch:ty) => {
        pub(crate) fn $name(
            function: &PsiOptimizationFunction,
            patch: $patch,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            reconstruct_removed_result_use_accounting(
                function,
                patch.location,
                patch.source_operation,
                patch.result,
                ScalarType::Integer(patch.scalar_type),
            )
        }
    };
}

scalar_identity_reconstruction!(
    reconstruct_proof_certified_scalar_identity_accounting,
    ProofCertifiedScalarIdentityRewrite
);

scalar_identity_reconstruction!(
    reconstruct_total_scalar_identity_accounting,
    TotalScalarIdentityRewrite
);
