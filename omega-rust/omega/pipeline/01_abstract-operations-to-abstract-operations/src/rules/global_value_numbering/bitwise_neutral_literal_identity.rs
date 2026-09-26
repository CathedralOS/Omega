//! The exact bitwise neutral-literal rule, and the closed semantic partition
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
pub struct BitwiseNeutralLiteralIdentityRule;

impl BitwiseNeutralLiteralIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for BitwiseNeutralLiteralIdentityRule {
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

/// Return the six exact-width bitwise neutral laws in canonical operation and
/// left-literal/right-literal order.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
            all_ones(*scalar_type),
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
            zero(*scalar_type),
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroRight,
            zero(*scalar_type),
        ),
        _ => Vec::new(),
    }
}

fn pair(
    source_operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    scalar_type: IntegerType,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
    left_identity: TotalScalarIdentityKind,
    right_identity: TotalScalarIdentityKind,
    law_value: IntegerValue,
) -> Vec<TotalScalarIdentityShape> {
    vec![
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: right,
            law_operand: left,
            scalar_type,
            law_operand_type: scalar_type,
            identity: left_identity,
            expected_law_value: law_value,
        },
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            law_operand: right,
            scalar_type,
            law_operand_type: scalar_type,
            identity: right_identity,
            expected_law_value: law_value,
        },
    ]
}

fn all_ones(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    }
}

const fn zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
