//! Exact multiplication-by-zero tests.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::id;
use crate::rules::tests::fixtures::proof_check_elision::{
    live_divide_by_one_unit, live_exact_multiply_by_zero_unit,
};
use abstract_operations::AbstractOperation as O;
use optimization_core::ScalarConstantFactIdentity;
use optimization_unit::{
    OptimizationFact, ProofCertifiedScalarIdentityKind, ProofCertifiedScalarIdentityRewrite,
    PsiRewriteCandidate, PsiRewritePatch, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_proof_certified_scalar_identity_candidate,
    validate_psi_optimization_unit,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};

#[test]
fn proof_certified_exact_multiply_by_zero_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_multiply_by_zero_unit(integer, true);
    let contract = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    let (constant_fact, obligation_fact) =
        candidate.proof_certified_scalar_identity_witness().unwrap();

    for identity in [
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
    ] {
        let forged = PsiRewriteCandidate::new_proof_certified_scalar_identity(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            constant_fact,
            obligation_fact,
            candidate.predicted_cost_delta(),
            ProofCertifiedScalarIdentityRewrite { identity, ..patch },
        )
        .unwrap();
        assert_eq!(
            validate_proof_certified_scalar_identity_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        );
    }
    let wrong_replacement = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        obligation_fact,
        candidate.predicted_cost_delta(),
        ProofCertifiedScalarIdentityRewrite {
            replacement: id(323, ValueId::new),
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &wrong_replacement),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
    let foreign_constant = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        ScalarConstantFactIdentity::from_canonical_bytes(b"foreign zero-product literal"),
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_constant),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    );
    let foreign_obligation = PsiRewriteCandidate::new_proof_certified_scalar_identity(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        constant_fact,
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign zero-product proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_scalar_identity_candidate(&unit, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let nonzero = live_divide_by_one_unit(
        integer,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    let mut manager = crate::AnalysisManager::new(&nonzero);
    let products = manager
        .require_all(&nonzero, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&nonzero, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut missing_proof = live_exact_multiply_by_zero_unit(integer, false);
    missing_proof.accepted_obligation_facts.clear();
    missing_proof.identity = recompute_psi_optimization_unit_identity(&missing_proof);
    let mut manager = crate::AnalysisManager::new(&missing_proof);
    let products = manager
        .require_all(&missing_proof, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&missing_proof, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut unused = live_exact_multiply_by_zero_unit(integer, false);
    let O::Return { value, .. } = &mut unused.functions[0].blocks[0].nodes[2].operation else {
        unreachable!()
    };
    *value = id(324, ValueId::new);
    unused.functions[0].blocks[0].nodes[2].uses[0].value = id(324, ValueId::new);
    unused.identity = recompute_psi_optimization_unit_identity(&unused);
    validate_psi_optimization_unit(&unused).unwrap();
    let mut manager = crate::AnalysisManager::new(&unused);
    let products = manager
        .require_all(&unused, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
            .propose(&unused, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut detached = unit.clone();
    detached.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    detached.identity = recompute_psi_optimization_unit_identity(&detached);
    assert!(matches!(
        validate_psi_optimization_unit(&detached),
        Err(OptimizationUnitValidationError::FactIndexMismatch(_))
    ));
}
