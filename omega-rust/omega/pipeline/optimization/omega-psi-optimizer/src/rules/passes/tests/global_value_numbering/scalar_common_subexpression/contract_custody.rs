//! Exact contract, relabelling, cost, and validator custody for GVN CSE rows 0--8.

use omega_optimization_core::{AnalysisInvalidationSet, AnalysisSet};

use super::*;

#[derive(Clone, Copy)]
enum ValidationRoute {
    SameBlock,
    Dominating,
    PhiTranslated,
}

struct Case {
    unit: PsiOptimizationUnit,
    candidate: PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    route: ValidationRoute,
    validator: OptimizationValidatorIdentity,
}

fn case(
    unit: PsiOptimizationUnit,
    rule: impl PsiOptimizationRule,
    route: ValidationRoute,
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
    let [candidate] = rule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("representative GVN fixture proposes one candidate");
    Case {
        unit,
        candidate,
        contract,
        route,
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator_domain),
    }
}

fn cases() -> Vec<Case> {
    vec![
        case(
            local_cse_unit(),
            SameBlockTotalScalarCseRule,
            ValidationRoute::SameBlock,
            b"omega.validator.same-block-obligation-free-total-scalar-cse.v1",
        ),
        case(
            proof_certified_local_cse_unit(),
            SameBlockProofCertifiedScalarCseRule,
            ValidationRoute::SameBlock,
            b"omega.validator.same-block-proof-certified-total-scalar-cse.v1",
        ),
        case(
            dominator_gvn_unit(),
            DominatorTotalScalarGvnRule,
            ValidationRoute::Dominating,
            b"omega.validator.dominator-total-scalar-cse.v1",
        ),
        case(
            proof_certified_dominator_gvn_unit(),
            DominatorProofCertifiedScalarGvnRule,
            ValidationRoute::Dominating,
            b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1",
        ),
        case(
            phi_translated_gvn_unit(),
            PhiTranslatedObligationFreeScalarGvnRule,
            ValidationRoute::PhiTranslated,
            b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1",
        ),
        case(
            proof_certified_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedScalarGvnRule,
            ValidationRoute::PhiTranslated,
            b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1",
        ),
        case(
            compatible_policy_local_cse_unit(),
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
            ValidationRoute::SameBlock,
            b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1",
        ),
        case(
            compatible_policy_dominator_gvn_unit(),
            DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
            ValidationRoute::Dominating,
            b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
        ),
        case(
            compatible_policy_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
            ValidationRoute::PhiTranslated,
            b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        ),
    ]
}

fn validate(
    case: &Case,
    candidate: &PsiRewriteCandidate,
) -> Result<omega_optimization_validation::ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match case.route {
        ValidationRoute::SameBlock => {
            validate_local_scalar_common_subexpression_candidate(&case.unit, candidate)
        }
        ValidationRoute::Dominating => {
            validate_dominating_scalar_common_subexpression_candidate(&case.unit, candidate)
        }
        ValidationRoute::PhiTranslated => {
            validate_phi_translated_scalar_common_subexpression_candidate(&case.unit, candidate)
        }
    }
}

fn rebuilt(
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    predicted_cost_delta: i64,
) -> PsiRewriteCandidate {
    let proof_fact = candidate.accepted_obligation_witness().unwrap_or_else(|| {
        omega_optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"synthetic GVN relabelling witness",
        )
    });
    match candidate.patch() {
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
            if contract.safety_class() == OptimizationSafetyClass::ProofCertified {
                PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    proof_fact,
                    predicted_cost_delta,
                    patch,
                )
            } else {
                PsiRewriteCandidate::new_local_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    predicted_cost_delta,
                    patch,
                )
            }
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
            if contract.safety_class() == OptimizationSafetyClass::ProofCertified {
                PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    proof_fact,
                    predicted_cost_delta,
                    patch,
                )
            } else {
                PsiRewriteCandidate::new_dominating_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    predicted_cost_delta,
                    patch,
                )
            }
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
            if contract.safety_class() == OptimizationSafetyClass::ProofCertified {
                PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    proof_fact,
                    predicted_cost_delta,
                    patch,
                )
            } else {
                PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                    candidate.input(),
                    contract,
                    candidate.affected_blocks().to_vec(),
                    candidate.provenance().to_vec(),
                    predicted_cost_delta,
                    patch,
                )
            }
        }
        _ => panic!("GVN CSE case uses its scope-specific patch"),
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
fn every_gvn_cse_rule_binds_exact_contract_cost_and_validator_identity() {
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
            validate(&case, &case.candidate).unwrap().validator(),
            case.validator
        );
    }
}

#[test]
fn every_gvn_cse_rule_rejects_all_cross_rule_and_contract_corruption() {
    let cases = cases();
    let contracts = cases.iter().map(|case| case.contract).collect::<Vec<_>>();

    for (case_index, case) in cases.iter().enumerate() {
        for (contract_index, contract) in contracts.iter().copied().enumerate() {
            if case_index == contract_index {
                continue;
            }
            assert!(validate(case, &rebuilt(&case.candidate, contract, -1)).is_err());
        }

        let base = case.contract;
        let required_superset = match case.route {
            ValidationRoute::SameBlock => AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            ValidationRoute::Dominating | ValidationRoute::PhiTranslated => AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
        };
        let wrong_safety = if base.safety_class() == OptimizationSafetyClass::ProofCertified {
            OptimizationSafetyClass::ExactOperationSemantics
        } else {
            OptimizationSafetyClass::ProofCertified
        };
        let corrupt_contracts = [
            contract_with(
                base,
                OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule.unknown-gvn.v1"),
                base.required_analyses(),
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                required_superset,
                base.invalidated_analyses(),
                base.safety_class(),
            ),
            contract_with(
                base,
                base.identity(),
                AnalysisSet::new([AnalysisKind::UseDefinition]),
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
            assert!(validate(case, &rebuilt(&case.candidate, contract, -1)).is_err());
        }
        assert_eq!(
            validate(case, &rebuilt(&case.candidate, base, 0)),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
        );
    }
}
