//! Phi-translated leader construction and validation.

use super::*;

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
fn phi_translated_gvn_preserves_result_identity_and_reaches_fixed_point() {
    let unit = phi_translated_gvn_unit();
    let [candidate] = phi_translated_candidates(&unit)
        .try_into()
        .expect("both predecessor translations have available leaders");
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.parameter_position, 1);
    assert_eq!(patch.redundant_result, id(1_710, ValueId::new));
    assert_eq!(
        patch
            .incoming
            .iter()
            .map(|row| (row.edge, row.source, row.leader_result))
            .collect::<Vec<_>>(),
        [
            (
                id(1_717, EdgeId::new),
                id(1_705, BlockId::new),
                id(1_712, ValueId::new),
            ),
            (
                id(1_720, EdgeId::new),
                id(1_703, BlockId::new),
                id(1_711, ValueId::new),
            ),
        ]
    );
    assert!(candidate.substitutions().is_empty());
    assert!(candidate.consumed_facts().is_empty());

    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    let join = &output.functions[0].blocks[0];
    assert_eq!(join.parameters.len(), 2);
    assert_eq!(join.parameters[1].value, id(1_710, ValueId::new));
    assert_eq!(join.nodes.len(), 1);
    assert!(
        matches!(join.nodes[0].operation, O::Return { value, .. } if value == id(1_710, ValueId::new))
    );
    for (source, leader) in [
        (id(1_703, BlockId::new), id(1_711, ValueId::new)),
        (id(1_705, BlockId::new), id(1_712, ValueId::new)),
    ] {
        let edge = output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == source)
            .unwrap()
            .nodes
            .last()
            .unwrap()
            .successors
            .first()
            .unwrap();
        assert_eq!(edge.bindings.len(), 2);
        assert_eq!(edge.bindings[1].parameter, id(1_710, ValueId::new));
        assert_eq!(edge.bindings[1].argument, leader);
    }
    assert!(phi_translated_candidates(output).is_empty());

    let mut corrupted_patch = patch;
    corrupted_patch.incoming[0].leader_result = id(1_711, ValueId::new);
    let corrupted = PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
        unit.identity,
        PhiTranslatedObligationFreeScalarGvnRule::contract(),
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        candidate.predicted_cost_delta(),
        corrupted_patch,
    )
    .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &corrupted),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );
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
fn phi_translated_gvn_candidate_rejects_noncanonical_incoming_order() {
    let unit = phi_translated_gvn_unit();
    let [candidate] = phi_translated_candidates(&unit).try_into().unwrap();
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(mut patch) =
        candidate.patch()
    else {
        unreachable!()
    };
    patch.incoming.reverse();
    assert_eq!(
        PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedObligationFreeScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.predicted_cost_delta(),
            patch,
        ),
        Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    );
}

#[test]
fn proof_certified_phi_translation_consumes_only_redundant_evidence() {
    let unit = proof_certified_phi_translated_gvn_unit();
    assert!(phi_translated_candidates(&unit).is_empty());
    let [candidate] = proof_certified_phi_translated_candidates(&unit)
        .try_into()
        .expect("all three exact-add operations retain accepted evidence");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_713, OperationId::new))
        .unwrap()
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(
        candidate.consumed_facts(),
        [omega_optimization_core::OptimizationFactReference::AcceptedObligation(redundant_fact,),]
    );
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(
        patch
            .incoming
            .iter()
            .map(|row| row.leader_operation)
            .collect::<Vec<_>>(),
        [id(1_716, OperationId::new), id(1_715, OperationId::new),]
    );
    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1",
        )
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
    }));
    assert!(proof_certified_phi_translated_candidates(accepted.unit()).is_empty());

    let foreign =
        PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
            unit.identity,
            PhiTranslatedProofCertifiedScalarGvnRule::contract(),
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"foreign proof phi fact",
            ),
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &foreign),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
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
fn compatible_policy_phi_translation_preserves_result_and_consumes_only_redundant_evidence() {
    let unit = compatible_policy_phi_translated_gvn_unit();
    let [candidate] = compatible_policy_phi_translated_candidates(&unit)
        .try_into()
        .expect("wrapping and saturating arm leaders are compatible");
    assert!(phi_translated_candidates(&unit).is_empty());
    assert!(proof_certified_phi_translated_candidates(&unit).is_empty());
    let redundant_fact = unit.accepted_obligation_facts[0].identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    assert_eq!(candidate.substitutions(), []);
    assert_eq!(candidate.consumed_facts().len(), 1);
    let accepted =
        validate_phi_translated_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        )
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_713, OperationId::new))
    }));
    let join = &accepted.unit().functions[0].blocks[0];
    assert_eq!(
        join.parameters.last().unwrap().value,
        id(1_710, ValueId::new)
    );
    assert_eq!(join.nodes.len(), 1);
    assert!(compatible_policy_phi_translated_candidates(accepted.unit()).is_empty());
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
            omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
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
        Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    );
}
