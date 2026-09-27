//! General proof-certified scalar identity tests.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::LiveProofCertifiedIntegerIdentityEliminationRule;
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::id;
use crate::rules::tests::fixtures::proof_check_elision::{
    exact_add_unit, live_exact_add_zero_unit,
};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    OptimizationFact, ProofCertifiedScalarIdentityKind, PsiRewritePatch,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    validate_proof_certified_scalar_identity_candidate, validate_psi_optimization_unit,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ObligationId, OperationId, ValueId,
};

#[test]
fn live_proof_certified_identity_vocabulary_is_closed_and_canonical() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let left = id(303, ValueId::new);
    let right = id(304, ValueId::new);
    let result = id(305, ValueId::new);
    let source = id(308, OperationId::new);
    let obligation = id(309, ObligationId::new);
    let rows = [
        (
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(7),
            O::ExactIntegerAdd {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerAdd {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerSubtract {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight,
        ),
        (
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            O::ExactIntegerMultiply {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(1),
            O::ExactIntegerMultiply {
                psi_operation: source,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerShiftLeft {
                psi_operation: source,
                obligation,
                result,
                value_type: integer,
                count_type: integer,
                value: left,
                count: right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount,
        ),
        (
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(0),
            O::ExactIntegerShiftRight {
                psi_operation: source,
                obligation,
                result,
                value_type: integer,
                count_type: integer,
                value: left,
                count: right,
            },
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount,
        ),
    ];
    for (left_constant, right_constant, operation, expected) in rows {
        let mut unit = exact_add_unit();
        for (index, constant) in [left_constant, right_constant].into_iter().enumerate() {
            let O::IntegerConstant { value, .. } =
                &mut unit.functions[0].blocks[0].nodes[index].operation
            else {
                unreachable!()
            };
            *value = constant;
            let defined = [left, right][index];
            for fact in &mut unit.functions[0].facts {
                if let OptimizationFact::IntegerConstant {
                    value,
                    constant: row,
                    ..
                } = fact
                    && *value == defined
                {
                    *row = constant;
                }
            }
        }
        unit.functions[0].blocks[0].nodes[2].operation = operation;
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        validate_psi_optimization_unit(&unit).unwrap();
        let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let [candidate] = LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .try_into()
            .unwrap();
        let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
        validate_proof_certified_scalar_identity_candidate(&unit, &candidate).unwrap();
    }

    let mut missing_proof = live_exact_add_zero_unit();
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let contract = LiveProofCertifiedIntegerIdentityEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&missing_proof);
    let products = manager
        .require_all(&missing_proof, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedIntegerIdentityEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}
