//! Cross-block dominating-leader behavior.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::{
    DominatorProofCertifiedScalarGvnRule, DominatorTotalScalarGvnRule, node_elision_accounting,
};
use crate::RuleAnalysisView;
use crate::rules::tests::fixtures::global_value_numbering::{
    proof_certified_dominator_gvn_unit, sibling_only_gvn_unit,
};
use crate::rules::tests::fixtures::id;
use optimization_core::OptimizationSafetyClass;
use optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite, NodeLocation, OptimizationFact,
    PsiRewriteCandidate, PsiRewritePatch, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_dominating_scalar_common_subexpression_candidate,
};
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, OperationId, ScalarType, ValueId};

#[test]
fn proof_certified_dominator_gvn_consumes_cross_block_redundant_evidence() {
    let unit = proof_certified_dominator_gvn_unit();
    let contract = DominatorProofCertifiedScalarGvnRule::contract();
    assert_eq!(
        contract.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = DominatorProofCertifiedScalarGvnRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("the entry exact add dominates one proof-certified duplicate");
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == id(1_351, OperationId::new))
        .expect("fixture retains the dominated operation fact")
        .identity;
    assert_eq!(
        candidate.accepted_obligation_witness(),
        Some(redundant_fact)
    );
    let PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        unreachable!()
    };
    assert_eq!(patch.leader.block, id(1_343, BlockId::new));
    assert_eq!(patch.redundant.block, id(1_342, BlockId::new));
    assert_eq!(patch.leader_operation, id(1_349, OperationId::new));
    assert_eq!(patch.redundant_operation, id(1_351, OperationId::new));
    let accepted =
        validate_dominating_scalar_common_subexpression_candidate(&unit, &candidate).unwrap();
    assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert!(accepted.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == id(1_351, OperationId::new)
        )
    }));

    let forged = PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"foreign proof-certified dominator GVN fact",
        ),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    );

    for missing_operation in [patch.leader_operation, patch.redundant_operation] {
        let mut missing = unit.clone();
        missing
            .accepted_obligation_facts
            .retain(|fact| fact.operation != missing_operation);
        missing.identity = recompute_psi_optimization_unit_identity(&missing);
        let mut manager = crate::AnalysisManager::new(&missing);
        let products = manager
            .require_all(&missing, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            DominatorProofCertifiedScalarGvnRule
                .propose(&missing, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );
        let forged =
            PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                missing.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance().to_vec(),
                redundant_fact,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
        assert_eq!(
            validate_dominating_scalar_common_subexpression_candidate(&missing, &forged),
            Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        );
    }
}

#[test]
fn dominator_gvn_rejects_an_equivalent_sibling_expression_at_a_join() {
    let unit = sibling_only_gvn_unit();
    let contract = DominatorTotalScalarGvnRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        DominatorTotalScalarGvnRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let function = &unit.functions[0];
    let leader = NodeLocation {
        machine: function.machine,
        block: id(1_443, BlockId::new),
        node: 0,
    };
    let redundant = NodeLocation {
        machine: function.machine,
        block: id(1_442, BlockId::new),
        node: 0,
    };
    let (affected, provenance) =
        node_elision_accounting(function, redundant, id(1_449, ValueId::new)).unwrap();
    let forged = PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
        unit.identity,
        contract,
        affected,
        provenance,
        -1,
        DominatingScalarCommonSubexpressionRewrite {
            leader,
            redundant,
            leader_operation: id(1_452, OperationId::new),
            redundant_operation: id(1_450, OperationId::new),
            leader_result: id(1_448, ValueId::new),
            redundant_result: id(1_449, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(
        validate_dominating_scalar_common_subexpression_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}
