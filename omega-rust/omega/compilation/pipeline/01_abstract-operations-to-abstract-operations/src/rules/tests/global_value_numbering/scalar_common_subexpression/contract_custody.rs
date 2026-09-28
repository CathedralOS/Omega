//! Exact contract, relabelling, cost, and validator custody for GVN CSE rows 0--8.

use super::super::super::super::{
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule, PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule, SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    SameBlockProofCertifiedScalarCseRule, SameBlockTotalScalarCseRule,
};
use crate::rules::tests::fixtures::global_value_numbering::{
    compatible_policy_dominator_gvn_unit, compatible_policy_local_cse_unit,
    compatible_policy_phi_translated_gvn_unit, dominator_gvn_unit, local_cse_unit,
    phi_translated_gvn_unit, proof_certified_dominator_gvn_unit, proof_certified_local_cse_unit,
    proof_certified_phi_translated_gvn_unit,
};
use crate::{PsiOptimizationRule, RuleAnalysisView};
use optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_dominating_scalar_common_subexpression_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
};

use optimization_core::{AnalysisInvalidationSet, AnalysisSet};

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
}

fn case(unit: PsiOptimizationUnit, rule: impl PsiOptimizationRule, route: ValidationRoute) -> Case {
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
    }
}

fn cases() -> Vec<Case> {
    vec![
        case(
            local_cse_unit(),
            SameBlockTotalScalarCseRule,
            ValidationRoute::SameBlock,
        ),
        case(
            proof_certified_local_cse_unit(),
            SameBlockProofCertifiedScalarCseRule,
            ValidationRoute::SameBlock,
        ),
        case(
            dominator_gvn_unit(),
            DominatorTotalScalarGvnRule,
            ValidationRoute::Dominating,
        ),
        case(
            proof_certified_dominator_gvn_unit(),
            DominatorProofCertifiedScalarGvnRule,
            ValidationRoute::Dominating,
        ),
        case(
            phi_translated_gvn_unit(),
            PhiTranslatedObligationFreeScalarGvnRule,
            ValidationRoute::PhiTranslated,
        ),
        case(
            proof_certified_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedScalarGvnRule,
            ValidationRoute::PhiTranslated,
        ),
        case(
            compatible_policy_local_cse_unit(),
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
            ValidationRoute::SameBlock,
        ),
        case(
            compatible_policy_dominator_gvn_unit(),
            DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
            ValidationRoute::Dominating,
        ),
        case(
            compatible_policy_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
            ValidationRoute::PhiTranslated,
        ),
    ]
}

fn validate(
    case: &Case,
    candidate: &PsiRewriteCandidate,
) -> Result<optimization_unit_semantics::ValidatedPsiRewrite, OptimizationUnitValidationError> {
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
        optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
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
