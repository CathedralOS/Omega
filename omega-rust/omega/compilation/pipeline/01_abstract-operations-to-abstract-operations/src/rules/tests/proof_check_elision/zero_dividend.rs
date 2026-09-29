//! Zero-dividend tests.
use crate::rules::registry::PsiOptimizationRule;
use abstract_operations::{
    AbstractOperation as O, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
};

use super::super::super::LiveProofCertifiedIntegerZeroDividendEliminationRule;
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::id;
use crate::rules::tests::fixtures::proof_check_elision::live_zero_dividend_unit;
use optimization_core::{OptimizationFactReference, OptimizationValidatorIdentity};
use optimization_unit::{OptimizationFact, ProofCertifiedScalarIdentityKind, PsiRewritePatch};
use optimization_unit_semantics::validate_proof_certified_scalar_identity_candidate;
use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};

#[test]
fn proof_certified_zero_dividend_covers_divide_remainder_policies_and_signs() {
    let rows = [
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
    ];
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (policy, expected) in rows.into_iter().enumerate() {
            let unit = live_zero_dividend_unit(
                integer,
                |psi_operation, obligation, result, scalar_type, left, right| match policy {
                    0 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Exact,
                            IntegerArithmeticOperator::Divide,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    1 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Wrapping,
                            IntegerArithmeticOperator::Divide,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    2 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Saturating,
                            IntegerArithmeticOperator::Divide,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    3 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Exact,
                            IntegerArithmeticOperator::Remainder,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    4 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Wrapping,
                            IntegerArithmeticOperator::Remainder,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    5 => O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Saturating,
                            IntegerArithmeticOperator::Remainder,
                            Some(obligation),
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    _ => unreachable!(),
                },
            );
            let contract = LiveProofCertifiedIntegerZeroDividendEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerZeroDividendEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each proof-certified zero dividend has one candidate");
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            assert_eq!(patch.replacement, id(324, ValueId::new));
            assert_eq!(candidate.consumed_facts().len(), 2);
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::ScalarConstant(_)))
            );
            assert!(
                candidate
                    .consumed_facts()
                    .iter()
                    .any(|fact| matches!(fact, OptimizationFactReference::AcceptedObligation(_)))
            );

            let accepted =
                validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1"
                )
            );
            assert_eq!(
                accepted.unit().accepted_obligation_facts,
                unit.accepted_obligation_facts
            );
            assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            }));
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[1].operation,
                O::Return { value, .. } if value == id(324, ValueId::new)
            ));
            assert_eq!(
                accepted.unit().functions[0].blocks[0].nodes[1]
                    .provenance
                    .len(),
                2
            );
            assert_eq!(
                accepted.unit().functions[0].blocks[0].nodes[1].fuel.len(),
                2
            );

            let mut manager = crate::AnalysisManager::new(accepted.unit());
            let products = manager
                .require_all(accepted.unit(), contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                LiveProofCertifiedIntegerZeroDividendEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
