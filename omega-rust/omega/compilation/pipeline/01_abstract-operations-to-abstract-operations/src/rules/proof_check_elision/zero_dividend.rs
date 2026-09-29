//! Proof-certified zero dividends.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
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
    ProofCertifiedScalarIdentityShape, integer_zero, propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let (source_operation, result, scalar_type, left, identity) = match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            ..
        } => {
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            use ProofCertifiedScalarIdentityKind as Kind;
            let identity = match (arithmetic.policy(), arithmetic.operator()) {
                (Policy::Exact, Operator::Divide) => Kind::ExactIntegerDivideZeroLeft,
                (Policy::Wrapping, Operator::Divide) => Kind::WrappingIntegerDivideZeroLeft,
                (Policy::Saturating, Operator::Divide) => Kind::SaturatingIntegerDivideZeroLeft,
                (Policy::Exact, Operator::Remainder) => Kind::ExactIntegerRemainderZeroLeft,
                (Policy::Wrapping, Operator::Remainder) => Kind::WrappingIntegerRemainderZeroLeft,
                (Policy::Saturating, Operator::Remainder) => {
                    Kind::SaturatingIntegerRemainderZeroLeft
                }
                (_, Operator::Add | Operator::Subtract | Operator::Multiply) => {
                    return Vec::new();
                }
            };
            (*psi_operation, *result, *scalar_type, *left, identity)
        }
        _ => return Vec::new(),
    };
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        integer_zero(scalar_type),
    )]
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LiveProofCertifiedIntegerZeroDividendEliminationRule;

impl LiveProofCertifiedIntegerZeroDividendEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
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

impl PsiOptimizationRule for LiveProofCertifiedIntegerZeroDividendEliminationRule {
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
            proof_certified_integer_zero_dividend_shapes,
        )
    }
}
