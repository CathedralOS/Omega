//! Phi-translated leader construction and validation.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::{
    PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule,
};
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::global_value_numbering::{
    PhiTranslatedRightArm, compatible_policy_phi_translated_gvn_unit, phi_translated_gvn_fixture,
    proof_certified_phi_translated_gvn_unit,
};
use crate::rules::tests::fixtures::id;
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_phi_translated_scalar_common_subexpression_candidate,
};
use semantic_vocabulary::{OperationId, ValueId};

fn phi_translated_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedObligationFreeScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedObligationFreeScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn proof_certified_phi_translated_candidates(
    unit: &PsiOptimizationUnit,
) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedProofCertifiedScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedProofCertifiedScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn compatible_policy_phi_translated_candidates(
    unit: &PsiOptimizationUnit,
) -> Vec<PsiRewriteCandidate> {
    let contract = PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn phi_translated_gvn_requires_a_typed_leader_on_every_incoming_arm() {
    for right_arm in [
        PhiTranslatedRightArm::Missing,
        PhiTranslatedRightArm::MismatchedType,
    ] {
        let unit = phi_translated_gvn_fixture(right_arm, false, false);
        assert!(phi_translated_candidates(&unit).is_empty());
    }
}

#[test]
fn proof_certified_phi_translation_requires_every_leader_fact() {
    let original = proof_certified_phi_translated_gvn_unit();
    let [candidate] = proof_certified_phi_translated_candidates(&original)
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let redundant_fact = candidate.accepted_obligation_witness().unwrap();
    let mut unit = original;
    unit.accepted_obligation_facts
        .retain(|fact| fact.operation != id(1_716, OperationId::new));
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
    let detached_leader =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedProofCertifiedScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            redundant_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &detached_leader,),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
}

#[test]
fn compatible_policy_phi_translation_declines_incomplete_arms_and_rejects_corruption() {
    for right_arm in [
        PhiTranslatedRightArm::Missing,
        PhiTranslatedRightArm::MismatchedType,
    ] {
        let unit = phi_translated_gvn_fixture(right_arm, false, true);
        assert!(compatible_policy_phi_translated_candidates(&unit).is_empty());
    }

    let original = compatible_policy_phi_translated_gvn_unit();
    let [candidate] = compatible_policy_phi_translated_candidates(&original)
        .try_into()
        .unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };

    let mut missing_fact = original.clone();
    missing_fact.accepted_obligation_facts.clear();
    missing_fact.identity = recompute_psi_optimization_unit_identity(&missing_fact);
    assert!(compatible_policy_phi_translated_candidates(&missing_fact).is_empty());

    let foreign_fact =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign compatible phi fact",
            ),
            candidate.predicted_cost_delta(),
            patch.clone(),
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &foreign_fact,),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut unavailable_patch = patch.clone();
    unavailable_patch.incoming[0].leader.node = 1;
    let unavailable =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            unavailable_patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &unavailable),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );

    let mut detached_patch = patch.clone();
    detached_patch.incoming[0].leader_operation = id(20_201, OperationId::new);
    detached_patch.incoming[0].leader_result = id(20_202, ValueId::new);
    let detached =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            detached_patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&original, &detached),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );

    let mut reordered_patch = patch;
    reordered_patch.incoming.reverse();
    assert_eq!(
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            original.identity,
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap(),
            candidate.predicted_cost_delta(),
            reordered_patch,
        ),
        Err(optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    );
}
