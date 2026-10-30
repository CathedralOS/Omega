//! Proof-certified exact signed right shifts of a negative-one value.

use abstract_operations::{AbstractOperation as O, IntegerShiftDirection, IntegerShiftPolicy};
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerValue};

use crate::rules::proof_check_elision::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn proof_certified_exact_signed_integer_negative_one_shift_right_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_shift() else {
        return Vec::new();
    };
    if !matches!(operands.policy, IntegerShiftPolicy::Exact { .. })
        || operands.direction != IntegerShiftDirection::Right
        || operands.value_type.carrier() != IntegerCarrier::Fixed
        || operands.value_type.sign() != IntegerSign::Signed
    {
        return Vec::new();
    }
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation: operands.psi_operation,
            result: operands.result,
            replacement: operands.value,
            identity_operand: operands.value,
            scalar_type: operands.value_type,
            identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
        },
        IntegerValue::Signed(-1),
    )]
}

psi_rule_contract!(
    LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
    super::proof_check_elision_rule_contract(
                b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
                AnalysisSet::new([
                    AnalysisKind::ScalarConstants,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
            ),
);

psi_rule_impl!(
    LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
    |unit, analyses| {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_exact_signed_integer_negative_one_shift_right_shapes,
        )
    }
);
