//! The exact wrapping multiply-zero rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use crate::rules::global_value_numbering::total_scalar_identities::{
    TotalScalarIdentityShape, exact_total_scalar_identity, propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingMultiplyZeroAnnihilationRule;

impl WrappingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingMultiplyZeroAnnihilationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(unit, analyses, Self::contract(), classify)
    }
}

/// Return left-zero then right-zero so ties remain deterministic.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    let O::WrappingIntegerMultiply {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    } = operation
    else {
        return Vec::new();
    };
    vec![
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            law_operand: *left,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
            expected_law_value: integer_zero(*scalar_type),
        },
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *right,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
            expected_law_value: integer_zero(*scalar_type),
        },
    ]
}

const fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
