//! General proof-certified integer identities.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::IntegerValue;

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::proof_check_elision::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, integer_one, integer_zero,
    propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn proof_certified_scalar_identity_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    use ProofCertifiedScalarIdentityKind as Kind;
    match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if arithmetic.policy() == IntegerOverflowPolicy::Exact => {
            let zero = integer_zero(*scalar_type);
            let one = integer_one(*scalar_type);
            // (replacement, identity operand, identity, identity constant),
            // left-literal law before right-literal law.
            let laws = match arithmetic.operator() {
                IntegerArithmeticOperator::Add => vec![
                    (*right, *left, Kind::ExactIntegerAddZeroLeft, zero),
                    (*left, *right, Kind::ExactIntegerAddZeroRight, zero),
                ],
                IntegerArithmeticOperator::Subtract => {
                    vec![(*left, *right, Kind::ExactIntegerSubtractZeroRight, zero)]
                }
                IntegerArithmeticOperator::Multiply => vec![
                    (*right, *left, Kind::ExactIntegerMultiplyOneLeft, one),
                    (*left, *right, Kind::ExactIntegerMultiplyOneRight, one),
                ],
                IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => {
                    Vec::new()
                }
            };
            laws.into_iter()
                .map(|(replacement, identity_operand, identity, constant)| {
                    (
                        ProofCertifiedScalarIdentityShape {
                            source_operation: *psi_operation,
                            result: *result,
                            replacement,
                            identity_operand,
                            scalar_type: *scalar_type,
                            identity,
                        },
                        constant,
                    )
                })
                .collect()
        }
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { .. },
            direction,
            value,
            count,
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: match direction {
                    IntegerShiftDirection::Left => Kind::ExactIntegerShiftLeftZeroCount,
                    IntegerShiftDirection::Right => Kind::ExactIntegerShiftRightZeroCount,
                },
            },
            integer_zero(*count_type),
        )],
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerIdentityEliminationRule;

impl LiveProofCertifiedIntegerIdentityEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for LiveProofCertifiedIntegerIdentityEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_proof_certified_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            proof_certified_scalar_identity_shapes,
        )
    }
}
