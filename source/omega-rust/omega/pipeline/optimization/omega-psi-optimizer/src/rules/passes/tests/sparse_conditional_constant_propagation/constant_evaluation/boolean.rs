//! Boolean-result constant-evaluation rule tests.

use super::*;

#[test]
fn boolean_not_and_equal_use_typed_boolean_patches() {
    let cases: [(bool, &dyn PsiOptimizationRule); 2] = [
        (false, &BooleanNotConstantsRule),
        (true, &BooleanEqualConstantsRule),
    ];
    for (equal, rule) in cases {
        let unit = boolean_unit(equal);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0].patch(),
            omega_optimization_unit::PsiRewritePatch::ReplaceBooleanOperationWithConstant(_)
        ));
        let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::BooleanConstant { value: false, .. }
        ));
    }
}

#[test]
fn integer_comparison_rules_reconstruct_operand_types_and_boolean_results() {
    let cases: [(ComparisonFixtureKind, &dyn PsiOptimizationRule, bool); 3] = [
        (
            ComparisonFixtureKind::Equal,
            &IntegerEqualConstantsRule,
            false,
        ),
        (
            ComparisonFixtureKind::LessThan,
            &IntegerLessThanConstantsRule,
            true,
        ),
        (
            ComparisonFixtureKind::LessOrEqual,
            &IntegerLessOrEqualConstantsRule,
            true,
        ),
    ];
    for (kind, rule, expected) in cases {
        let unit = integer_comparison_unit(kind);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::BooleanConstant { value, .. } if value == expected
        ));
    }
}
