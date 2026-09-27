//! Division-by-one tests.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::LiveProofCertifiedIntegerDivideByOneEliminationRule;
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::id;
use crate::rules::tests::fixtures::proof_check_elision::live_divide_by_one_unit;
use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationValidatorIdentity;
use optimization_unit::{OptimizationFact, ProofCertifiedScalarIdentityKind, PsiRewritePatch};
use optimization_unit_semantics::validate_proof_certified_scalar_identity_candidate;
use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};

#[test]
fn proof_certified_divide_by_one_covers_every_policy_and_integer_sign() {
    for integer in [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ] {
        for (policy, expected) in [
            (
                0,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
            ),
            (
                1,
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
            ),
            (
                2,
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
            ),
        ] {
            let unit = live_divide_by_one_unit(
                integer,
                |psi_operation, obligation, result, scalar_type, left, right| match policy {
                    0 => O::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    1 => O::WrappingIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    2 => O::SaturatingIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    _ => unreachable!(),
                },
            );
            let contract = LiveProofCertifiedIntegerDivideByOneEliminationRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let [candidate] = LiveProofCertifiedIntegerDivideByOneEliminationRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .try_into()
                .expect("each typed divide by literal one has one candidate");
            let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.identity, expected);
            assert_eq!(patch.replacement, id(323, ValueId::new));
            assert_eq!(candidate.consumed_facts().len(), 2);
            let accepted =
                validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
            assert_eq!(
                accepted.validator(),
                OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1"
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
                O::Return { value, .. } if value == id(323, ValueId::new)
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
                LiveProofCertifiedIntegerDivideByOneEliminationRule
                    .propose(accepted.unit(), RuleAnalysisView::new(&products))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
