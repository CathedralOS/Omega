//! Exact self-subtraction tests.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::{
    LiveProofCertifiedExactIntegerSelfSubtractEliminationRule, PROOF_CHECK_ELISION_PASS_NAME,
};
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::dead_scalar_elimination::discard_scalar_function_result;
use crate::rules::tests::fixtures::id;
use crate::rules::tests::fixtures::proof_check_elision::{
    live_exact_self_subtract_unit, live_proof_binary_identity_unit,
};
use abstract_operations::AbstractOperation as O;
use optimization_core::{
    OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, OptimizationFact,
    ProvenanceDisposition, ProvenanceRewrite, PsiRealizationSite, PsiRewriteCandidate,
    PsiRewritePatch, ScalarSubstitution, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError,
    validate_proof_certified_exact_integer_self_subtract_candidate, validate_psi_optimization_unit,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};

#[test]
fn proof_certified_exact_self_subtract_declines_and_rejects_corruption() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_self_subtract_unit(integer);
    let contract = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .unwrap();
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let obligation_fact = candidate.accepted_obligation_witness().unwrap();

    for forged_patch in [
        IntegerConstantRewrite {
            constant: IntegerValue::Unsigned(1),
            ..patch
        },
        IntegerConstantRewrite {
            source_operation: id(338, OperationId::new),
            ..patch
        },
        IntegerConstantRewrite {
            result: id(339, ValueId::new),
            ..patch
        },
        IntegerConstantRewrite {
            scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            constant: IntegerValue::Signed(0),
            ..patch
        },
    ] {
        let forged = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            forged_patch,
        )
        .unwrap();
        assert!(
            validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &forged).is_err()
        );
    }
    let foreign_fact = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign self-subtract proof",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_fact),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );
    let mut corrupt_provenance = candidate.provenance().to_vec();
    corrupt_provenance[0].fuel[0].units += 1;
    let corrupt_provenance = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        corrupt_provenance,
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &corrupt_provenance,),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
    let substituted = PsiRewriteCandidate::new_integer_evaluation(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        vec![ScalarSubstitution {
            from: patch.result,
            to: id(333, ValueId::new),
            scalar_type: ScalarType::Integer(integer),
        }],
        candidate.provenance().to_vec(),
        IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign self-subtract operand",
            ),
            obligation_fact,
        },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &substituted),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );
    let foreign_contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"foreign self-subtract rule"),
        OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
        1,
        contract.required_analyses(),
        contract.invalidated_analyses(),
        OptimizationSafetyClass::ProofCertified,
    )
    .unwrap();
    let foreign_rule = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        foreign_contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        obligation_fact,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &foreign_rule),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );
    let terminal_location = NodeLocation {
        machine: patch.location.machine,
        block: patch.location.block,
        node: 1,
    };
    let terminal = &unit.functions[0].blocks[0].nodes[1];
    let terminal_site = PsiRealizationSite::Node(terminal_location);
    let wrong_location = PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        vec![ProvenanceRewrite {
            input: terminal_site,
            disposition: ProvenanceDisposition::RealizedAt(terminal_site),
            sources: terminal.provenance.clone(),
            fuel: terminal.fuel.clone(),
        }],
        obligation_fact,
        candidate.predicted_cost_delta(),
        IntegerConstantRewrite {
            location: terminal_location,
            ..patch
        },
    )
    .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(&unit, &wrong_location),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let mut missing_catalog = unit.clone();
    missing_catalog.accepted_obligation_facts.clear();
    missing_catalog.identity = recompute_psi_optimization_unit_identity(&missing_catalog);
    let missing_catalog_candidate =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            missing_catalog.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(
            &missing_catalog,
            &missing_catalog_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    let mut wrong_policy = unit.clone();
    wrong_policy.functions[0].blocks[0].nodes[0].operation = O::WrappingIntegerSubtract {
        psi_operation: patch.source_operation,
        result: patch.result,
        scalar_type: integer,
        left: id(333, ValueId::new),
        right: id(333, ValueId::new),
    };
    wrong_policy.functions[0]
        .facts
        .retain(|fact| !matches!(fact, OptimizationFact::OperationObligationReference { .. }));
    wrong_policy.identity = recompute_psi_optimization_unit_identity(&wrong_policy);
    validate_psi_optimization_unit(&wrong_policy).unwrap();
    let wrong_policy_candidate =
        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
            wrong_policy.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            obligation_fact,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
    assert_eq!(
        validate_proof_certified_exact_integer_self_subtract_candidate(
            &wrong_policy,
            &wrong_policy_candidate,
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let unequal = live_proof_binary_identity_unit(
        integer,
        IntegerValue::Unsigned(1),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
    );
    for ineligible in [
        unequal,
        live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, _obligation, result, scalar_type, left, right| {
                O::WrappingIntegerSubtract {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        live_proof_binary_identity_unit(
            integer,
            IntegerValue::Unsigned(1),
            false,
            |psi_operation, _obligation, result, scalar_type, left, right| {
                O::SaturatingIntegerSubtract {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        discard_scalar_function_result(live_exact_self_subtract_unit(integer)),
    ] {
        let mut manager = crate::AnalysisManager::new(&ineligible);
        let products = manager
            .require_all(&ineligible, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(&ineligible, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }

    for mut missing in [
        live_exact_self_subtract_unit(integer),
        live_exact_self_subtract_unit(integer),
    ]
    .into_iter()
    .enumerate()
    {
        if missing.0 == 0 {
            missing.1.accepted_obligation_facts.clear();
        } else {
            missing.1.functions[0].facts.retain(|fact| {
                !matches!(fact, OptimizationFact::OperationObligationReference { .. })
            });
        }
        missing.1.identity = recompute_psi_optimization_unit_identity(&missing.1);
        let mut manager = crate::AnalysisManager::new(&missing.1);
        let products = manager
            .require_all(&missing.1, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule
                .propose(&missing.1, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
    }
}
