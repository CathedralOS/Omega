use super::*;

#[test]
fn integer_range_pair_comparisons_prove_only_universal_results() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u = IntegerValue::Unsigned;
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(7),
            u(7),
            u(7),
            u(7)
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(1),
            u(3),
            u(4),
            u(6)
        ),
        Some(false)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(1),
            u(4),
            u(3),
            u(6)
        ),
        None
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessThan,
            scalar_type,
            false,
            u(1),
            u(3),
            u(4),
            u(6)
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessThan,
            scalar_type,
            false,
            u(4),
            u(6),
            u(1),
            u(4)
        ),
        Some(false)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessOrEqual,
            scalar_type,
            false,
            u(1),
            u(4),
            u(4),
            u(6)
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessOrEqual,
            scalar_type,
            false,
            u(5),
            u(6),
            u(1),
            u(4)
        ),
        Some(false)
    );
    for (kind, expected) in [
        (IntegerRangePairComparisonKind::Equal, true),
        (IntegerRangePairComparisonKind::LessThan, false),
        (IntegerRangePairComparisonKind::LessOrEqual, true),
    ] {
        assert_eq!(
            evaluate_integer_range_pair_comparison(kind, scalar_type, true, u(1), u(6), u(1), u(6)),
            Some(expected)
        );
    }
}

#[test]
fn proof_derived_range_pair_proposes_two_fact_boolean_fold() {
    let unit = proof_range_pair_comparison_unit();
    let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
    let candidates = IntegerLessOrEqualRangeRangeRule
        .propose(&unit, RuleAnalysisView::new(&[ranges]))
        .unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("two applicable proof ranges produce one comparison candidate")
    };
    let (left_range_fact, right_range_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_range)
        .expect("range-pair candidate retains both proof facts");
    assert_ne!(left_range_fact, right_range_fact);
    assert_eq!(candidate.consumed_facts().len(), 2);
    assert!(matches!(
        candidate.patch(),
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(BooleanConstantRewrite {
            constant: true,
            ..
        })
    ));
    let accepted = validate_boolean_evaluation_candidate(&unit, candidate).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}
