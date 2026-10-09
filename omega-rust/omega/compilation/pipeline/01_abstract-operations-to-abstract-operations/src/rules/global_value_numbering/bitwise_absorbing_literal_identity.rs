//! The exact bitwise absorbing-literal rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ValueId};

use crate::rules::global_value_numbering::total_scalar_identities::{
    TotalScalarIdentityShape, all_ones, exact_total_scalar_identity,
    propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct BitwiseAbsorbingLiteralIdentityRule;

impl BitwiseAbsorbingLiteralIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1",
        )
    }
}

psi_optimization_rule_delegate!(
    BitwiseAbsorbingLiteralIdentityRule,
    propose = propose_total_scalar_identities,
    contract = Self::contract(),
    classify
);

/// Return the four exact-width absorbing laws in canonical operation and
/// left-literal/right-literal order. The law operand is also the replacement.
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
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
            scalar_type.zero(),
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
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
            all_ones(*scalar_type),
        ),
        _ => Vec::new(),
    }
}

fn pair(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    left_identity: TotalScalarIdentityKind,
    right_identity: TotalScalarIdentityKind,
    law_value: IntegerValue,
) -> Vec<TotalScalarIdentityShape> {
    vec![
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            law_operand: left,
            scalar_type,
            law_operand_type: scalar_type,
            identity: left_identity,
            expected_law_value: law_value,
        },
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: right,
            law_operand: right,
            scalar_type,
            law_operand_type: scalar_type,
            identity: right_identity,
            expected_law_value: law_value,
        },
    ]
}
