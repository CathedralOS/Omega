//! Boolean-result constant evaluation by exact rule identity.

use super::super::super::super::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, IntegerEqualConstantsRule,
    IntegerLessOrEqualConstantsRule, IntegerLessThanConstantsRule,
};
use crate::rules::tests::fixtures::sparse_conditional_constant_propagation::{
    BooleanFixtureKind, ComparisonFixtureKind, boolean_constant_unit,
    integer_comparison_constant_unit,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, compute_analysis};
use optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationRuleIdentity, ScalarConstantFactIdentity,
};
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiRewritePatch,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_boolean_evaluation_candidate,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

fn propose_one(unit: &PsiOptimizationUnit, rule: &dyn PsiOptimizationRule) -> PsiRewriteCandidate {
    let constants = compute_analysis(unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = rule
        .propose(unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    candidates.into_iter().next().unwrap()
}

fn forge_candidate(
    unit: &PsiOptimizationUnit,
    source: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    witness: IntegerEvaluationWitness,
    patch: BooleanConstantRewrite,
) -> PsiRewriteCandidate {
    PsiRewriteCandidate::new_boolean_evaluation(
        unit.identity,
        contract,
        source.affected_blocks().to_vec(),
        Vec::new(),
        source.provenance().to_vec(),
        witness,
        source.predicted_cost_delta(),
        patch,
    )
    .unwrap()
}

#[test]
fn boolean_validation_rejects_every_cross_rule_contract_and_unknown_identity() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let units = [
        boolean_constant_unit(BooleanFixtureKind::Not, true, false),
        boolean_constant_unit(BooleanFixtureKind::Equal, true, false),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::Equal,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessThan,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessOrEqual,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
    ];
    let rules: [&dyn PsiOptimizationRule; 5] = [
        &BooleanNotConstantsRule,
        &BooleanEqualConstantsRule,
        &IntegerEqualConstantsRule,
        &IntegerLessThanConstantsRule,
        &IntegerLessOrEqualConstantsRule,
    ];
    let contracts = [
        BooleanNotConstantsRule::contract(),
        BooleanEqualConstantsRule::contract(),
        IntegerEqualConstantsRule::contract(),
        IntegerLessThanConstantsRule::contract(),
        IntegerLessOrEqualConstantsRule::contract(),
    ];
    for (rule_index, (unit, rule)) in units.into_iter().zip(rules).enumerate() {
        let candidate = propose_one(&unit, rule);
        let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        for (contract_index, wrong_contract) in contracts.iter().copied().enumerate() {
            if contract_index == rule_index {
                continue;
            }
            let relabelled = forge_candidate(
                &unit,
                &candidate,
                wrong_contract,
                candidate.scalar_evaluation_witness().unwrap(),
                patch,
            );
            assert!(matches!(
                validate_boolean_evaluation_candidate(&unit, &relabelled),
                Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
            ));
        }
        let mut wrong_result_patch = patch;
        wrong_result_patch.constant = !wrong_result_patch.constant;
        let wrong_result = forge_candidate(
            &unit,
            &candidate,
            contracts[rule_index],
            candidate.scalar_evaluation_witness().unwrap(),
            wrong_result_patch,
        );
        assert!(matches!(
            validate_boolean_evaluation_candidate(&unit, &wrong_result),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));
    }

    let unit = boolean_constant_unit(BooleanFixtureKind::Not, true, false);
    let candidate = propose_one(&unit, &BooleanNotConstantsRule);
    let base = BooleanNotConstantsRule::contract();
    let unknown = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule.unknown-boolean-fold.v1"),
        base.pass(),
        base.version(),
        base.required_analyses(),
        base.invalidated_analyses(),
        base.safety_class(),
    )
    .unwrap();
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let relabelled = forge_candidate(
        &unit,
        &candidate,
        unknown,
        candidate.scalar_evaluation_witness().unwrap(),
        patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unit, &relabelled),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    ));
}

#[test]
fn boolean_validation_rejects_unary_and_binary_witness_fact_corruption() {
    let unary_unit = boolean_constant_unit(BooleanFixtureKind::Not, true, false);
    let unary = propose_one(&unary_unit, &BooleanNotConstantsRule);
    let IntegerEvaluationWitness::Unary { operand_fact } =
        unary.scalar_evaluation_witness().unwrap()
    else {
        unreachable!()
    };
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(unary_patch) = unary.patch() else {
        unreachable!()
    };
    let wrong_shape = forge_candidate(
        &unary_unit,
        &unary,
        BooleanNotConstantsRule::contract(),
        IntegerEvaluationWitness::Binary {
            left_fact: operand_fact,
            right_fact: operand_fact,
        },
        unary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unary_unit, &wrong_shape),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
    let wrong_fact = forge_candidate(
        &unary_unit,
        &unary,
        BooleanNotConstantsRule::contract(),
        IntegerEvaluationWitness::Unary {
            operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign boolean literal fact",
            ),
        },
        unary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unary_unit, &wrong_fact),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));

    let binary_unit = boolean_constant_unit(BooleanFixtureKind::Equal, true, false);
    let binary = propose_one(&binary_unit, &BooleanEqualConstantsRule);
    let IntegerEvaluationWitness::Binary { left_fact, .. } =
        binary.scalar_evaluation_witness().unwrap()
    else {
        unreachable!()
    };
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(binary_patch) = binary.patch() else {
        unreachable!()
    };
    let wrong_shape = forge_candidate(
        &binary_unit,
        &binary,
        BooleanEqualConstantsRule::contract(),
        IntegerEvaluationWitness::Unary {
            operand_fact: left_fact,
        },
        binary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&binary_unit, &wrong_shape),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
    let wrong_fact = forge_candidate(
        &binary_unit,
        &binary,
        BooleanEqualConstantsRule::contract(),
        IntegerEvaluationWitness::Binary {
            left_fact,
            right_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign equality literal fact",
            ),
        },
        binary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&binary_unit, &wrong_fact),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
}
