//! Exact three-family dead-scalar contract, relabelling, cost, and validator custody.

use optimization_core::{AnalysisInvalidationSet, AnalysisSet};

use super::*;

struct Case {
    unit: PsiOptimizationUnit,
    candidate: PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    validator: OptimizationValidatorIdentity,
}

fn case(
    unit: PsiOptimizationUnit,
    rule: impl PsiOptimizationRule,
    validator_domain: &[u8],
) -> Case {
    let contract = rule.contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = rule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .into_iter()
        .min_by_key(|candidate| candidate.node_decision_point())
        .expect("representative dead-scalar fixture proposes a candidate");
    Case {
        unit,
        candidate,
        contract,
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
    }
}

fn cases() -> Vec<Case> {
    vec![
        case(
            dead_scalar_literals_unit(),
            DeadScalarLiteralEliminationRule,
            b"omega.validator.dead-unused-scalar-literal.v1",
        ),
        case(
            dead_wrapping_add_unit(),
            DeadUnconditionallyTotalScalarEliminationRule,
            b"omega.validator.dead-unused-unconditionally-total-scalar.v1",
        ),
        case(
            dead_exact_add_unit(),
            ProofCertifiedDeadScalarEliminationRule,
            b"omega.validator.dead-unused-proof-certified-scalar-node.v1",
        ),
    ]
}

fn rebuilt(
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    predicted_cost_delta: i64,
) -> PsiRewriteCandidate {
    let PsiRewritePatch::RemoveDeadScalarNode(patch) = candidate.patch() else {
        panic!("dead-scalar family uses one exact patch")
    };
    if contract.safety_class() == OptimizationSafetyClass::ProofCertified {
        PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
            candidate.input(),
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            candidate.accepted_obligation_witness().unwrap_or_else(|| {
                optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                    b"synthetic dead-scalar relabelling witness",
                )
            }),
            predicted_cost_delta,
            patch,
        )
    } else {
        PsiRewriteCandidate::new_dead_scalar_node(
            candidate.input(),
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            predicted_cost_delta,
            patch,
        )
    }
    .unwrap()
}

fn contract_with(
    base: OptimizationRuleContract,
    identity: OptimizationRuleIdentity,
    required: AnalysisSet,
    invalidated: AnalysisInvalidationSet,
    safety: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        identity,
        base.pass(),
        base.version(),
        required,
        invalidated,
        safety,
    )
    .unwrap()
}

#[test]
fn dead_scalar_rosters_and_candidates_bind_complete_exact_contracts() {
    let dead = OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    assert_eq!(
        built_in_psi_registry(&dead)
            .unwrap()
            .contracts()
            .collect::<Vec<_>>(),
        [
            DeadScalarLiteralEliminationRule::contract(),
            DeadUnconditionallyTotalScalarEliminationRule::contract(),
        ]
    );
    let proof = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    assert_eq!(
        built_in_psi_registry(&proof).unwrap().contracts().next(),
        Some(ProofCertifiedDeadScalarEliminationRule::contract())
    );

    for case in cases() {
        assert_eq!(case.candidate.rule(), case.contract.identity());
        assert_eq!(
            case.candidate.required_analyses(),
            case.contract.required_analyses()
        );
        assert_eq!(
            case.candidate.invalidated_analyses(),
            case.contract.invalidated_analyses()
        );
        assert_eq!(case.candidate.safety_class(), case.contract.safety_class());
        assert_eq!(case.candidate.predicted_cost_delta(), -1);
        assert_eq!(
            validate_dead_scalar_node_candidate(&case.unit, &case.candidate)
                .unwrap()
                .validator(),
            case.validator
        );
    }
}

#[test]
fn every_dead_scalar_family_rejects_cross_rule_and_contract_corruption() {
    let cases = cases();
    let contracts = cases.iter().map(|case| case.contract).collect::<Vec<_>>();

    for (case_index, case) in cases.iter().enumerate() {
        for (contract_index, contract) in contracts.iter().copied().enumerate() {
            if case_index == contract_index {
                continue;
            }
            assert!(
                validate_dead_scalar_node_candidate(
                    &case.unit,
                    &rebuilt(&case.candidate, contract, -1),
                )
                .is_err()
            );
        }

        let base = case.contract;
        let wrong_safety = if base.safety_class() == OptimizationSafetyClass::ProofCertified {
            OptimizationSafetyClass::ExactOperationSemantics
        } else {
            OptimizationSafetyClass::ProofCertified
        };
        let corrupt_contracts = [
            contract_with(
                base,
                OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.unknown-dead-scalar.v1",
                ),
                base.required_analyses(),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                AnalysisSet::new([
                    AnalysisKind::ControlFlowGraph,
                    AnalysisKind::ValueLiveness,
                    AnalysisKind::EffectSummaries,
                ]),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                AnalysisSet::new([AnalysisKind::ValueLiveness]),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                base.required_analyses(),
                AnalysisInvalidationSet::new([
                    AnalysisKind::ControlFlowGraph,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                base.required_analyses(),
                AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                base.required_analyses(),
                base.invalidated_analyses(),
                wrong_safety,
            ),
        ];
        for contract in corrupt_contracts {
            assert_eq!(
                validate_dead_scalar_node_candidate(
                    &case.unit,
                    &rebuilt(&case.candidate, contract, -1),
                ),
                Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
            );
        }
        assert_eq!(
            validate_dead_scalar_node_candidate(&case.unit, &rebuilt(&case.candidate, base, 0),),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
        );
    }
}
