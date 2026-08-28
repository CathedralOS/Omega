use super::super::*;
use psi_core::{IntegerType, PropositionContext, ScalarType, ValueId};
use psi_proof_admission::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness,
    IntegerCorrelatedForbiddenRootWitness, IntegerCorrelatedForbiddenRootWitnessError,
    check_integer_correlated_forbidden_root_witness,
};

#[test]
fn affine_fork_join_replays_correlated_branches_without_importing_prefix_proofs() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let root_id = ValueId::new(1791).expect("fork root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("fork value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("fork literal")
    };
    let left_offset = value(1792);
    let left_product = value(1793);
    let right_offset = value(1794);
    let right_product = value(1795);
    let definitions = vec![
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(1))
                .expect("root + 1"),
        ),
        Proposition::Equal(
            left_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset, literal(2))
                .expect("left * 2"),
        ),
        Proposition::Equal(
            right_offset.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(1))
                .expect("root - 1"),
        ),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, right_offset, literal(3))
                .expect("right * 3"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    let expected =
        exact_integer_source_interval_obligation(integer_type, root.clone(), -6553, 6553);
    assert_eq!(
        exact_integer_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            ExactIntegerOffsetOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        Some(expected.clone()),
        "2 * (x + 1) + 3 * (x - 1) replays as 5 * x - 1",
    );
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &definitions,
            definitions.len(),
            &parameters,
        ),
        expected,
        "the ordinary exact-add dispatch selects the correlated fork",
    );

    let cancel_left_offset = value(1796);
    let cancel_left_product = value(1797);
    let cancel_right_offset = value(1798);
    let cancel_right_product = value(1799);
    let cancellation_definitions = vec![
        Proposition::Equal(
            cancel_left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(3))
                .expect("root + 3"),
        ),
        Proposition::Equal(
            cancel_left_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, cancel_left_offset, literal(-2))
                .expect("left * -2"),
        ),
        Proposition::Equal(
            cancel_right_offset.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(4))
                .expect("root - 4"),
        ),
        Proposition::Equal(
            cancel_right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, cancel_right_offset, literal(-2))
                .expect("right * -2"),
        ),
    ];
    assert_eq!(
        exact_integer_subtract_obligation(
            integer_type,
            cancel_left_product.clone(),
            cancel_right_product.clone(),
            &cancellation_definitions,
            cancellation_definitions.len(),
            &parameters,
        ),
        Proposition::Truth,
        "-2 * (x + 3) - -2 * (x - 4) is the join-local constant -14",
    );

    assert_eq!(
        exact_integer_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            left_product,
            ExactIntegerOffsetOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        None,
        "the two branch definition walks must be disjoint",
    );
    let reordered_definitions = vec![
        definitions[2].clone(),
        definitions[3].clone(),
        definitions[0].clone(),
        definitions[1].clone(),
    ];
    assert_eq!(
        exact_integer_affine_fork_join_obligation(
            integer_type,
            value(1793),
            value(1795),
            ExactIntegerOffsetOperation::Add,
            &reordered_definitions,
            reordered_definitions.len(),
            &parameters,
        ),
        None,
        "right-branch definitions cannot precede the ordered left branch",
    );
    assert_eq!(
        exact_integer_affine_fork_join_obligation(
            integer_type,
            cancel_left_product,
            cancel_right_product,
            ExactIntegerOffsetOperation::Subtract,
            &cancellation_definitions,
            cancellation_definitions.len(),
            &BTreeSet::new(),
        ),
        None,
        "a local or stale root cannot authorize the fork",
    );
}

#[test]
fn distinct_root_affine_fork_join_uses_only_canonical_signature_rectangles() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let left_root_id = ValueId::new(1801).expect("left root");
    let right_root_id = ValueId::new(1802).expect("right root");
    let left_root = ScalarTerm::value(left_root_id, ScalarType::Integer(integer_type));
    let right_root = ScalarTerm::value(right_root_id, ScalarType::Integer(integer_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("fork value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("fork literal")
    };
    let left_offset = value(1803);
    let left_product = value(1804);
    let right_offset = value(1805);
    let right_product = value(1806);
    let definitions = vec![
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, left_root.clone(), literal(1))
                .expect("left root + 1"),
        ),
        Proposition::Equal(
            left_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset, literal(2))
                .expect("left branch * 2"),
        ),
        Proposition::Equal(
            right_offset.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, right_root.clone(), literal(1))
                .expect("right root - 1"),
        ),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, right_offset, literal(3))
                .expect("right branch * 3"),
        ),
    ];
    let parameters = BTreeSet::from([left_root_id, right_root_id]);
    let loose_left = Proposition::LessOrEqual(literal(-200), left_root.clone());
    let left_lower = Proposition::LessOrEqual(literal(-100), left_root.clone());
    let left_upper = Proposition::LessOrEqual(left_root.clone(), literal(100));
    let right_lower = Proposition::LessOrEqual(literal(-100), right_root.clone());
    let right_upper = Proposition::LessOrEqual(right_root.clone(), literal(100));
    let mut bounded = definitions.clone();
    bounded.extend([
        loose_left,
        left_lower.clone(),
        left_upper.clone(),
        right_lower.clone(),
        right_upper.clone(),
    ]);
    let expected = canonical_conjunction(vec![left_lower, left_upper, right_lower, right_upper]);
    assert_eq!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            ExactIntegerOffsetOperation::Add,
            &bounded,
            definitions.len(),
            &parameters,
        ),
        Some(expected.clone()),
        "the tightest unary signature rectangle proves the bivariate sum",
    );
    assert_eq!(
        exact_integer_add_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        expected,
        "ordinary dispatch selects the distinct-root rectangle after same-root priority",
    );
    assert!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            ExactIntegerOffsetOperation::Subtract,
            &bounded,
            definitions.len(),
            &parameters,
        )
        .is_some(),
        "Minkowski subtraction reverses the right interval endpoints",
    );

    assert_eq!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            ExactIntegerOffsetOperation::Add,
            &definitions,
            definitions.len(),
            &parameters,
        ),
        None,
        "carrier defaults alone leave this output rectangle partially unsafe",
    );
    let relational = Proposition::LessOrEqual(
        left_root.clone(),
        ScalarTerm::exact_integer_subtract(
            integer_type,
            literal(i128::from(i16::MAX)),
            right_root.clone(),
        )
        .expect("MAX - right root"),
    );
    let mut relational_only = definitions.clone();
    relational_only.push(relational);
    assert_eq!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            ExactIntegerOffsetOperation::Add,
            &relational_only,
            definitions.len(),
            &parameters,
        ),
        None,
        "cross-root relational premises require a future polyhedral family",
    );
    assert_eq!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left_product.clone(),
            left_product,
            ExactIntegerOffsetOperation::Add,
            &bounded,
            definitions.len(),
            &parameters,
        ),
        None,
        "same-root and overlapping walks retain the existing family priority",
    );
    let reordered = vec![
        definitions[2].clone(),
        definitions[3].clone(),
        definitions[0].clone(),
        definitions[1].clone(),
    ];
    assert_eq!(
        exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            value(1804),
            value(1806),
            ExactIntegerOffsetOperation::Add,
            &reordered,
            reordered.len(),
            &parameters,
        ),
        None,
        "right-branch definitions cannot precede the left branch",
    );
}

#[test]
fn same_root_affine_product_join_uses_exact_discrete_quadratic_extrema() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let root_id = ValueId::new(1811).expect("quadratic root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("quadratic value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal =
        |value| ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal");
    let left = value(1812);
    let right = value(1813);
    let definitions = vec![
        Proposition::Equal(
            left.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(10))
                .expect("root + 10"),
        ),
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(10))
                .expect("root - 10"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    let lower = Proposition::LessOrEqual(literal(-5), root.clone());
    let upper = Proposition::LessOrEqual(root.clone(), literal(5));
    let mut bounded = definitions.clone();
    bounded.extend([
        Proposition::LessOrEqual(literal(-100), root.clone()),
        lower.clone(),
        upper.clone(),
    ]);
    let expected = canonical_conjunction(vec![lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_affine_quadratic_range(
            IntegerOffset::Nonnegative(1),
            IntegerOffset::Nonnegative(10),
            IntegerOffset::Nonnegative(1),
            IntegerOffset::Negative(10),
            (-5, 5),
        ),
        Some((-100, -75)),
        "correlated x² - 100 is tighter than the unsafe rectangle hull",
    );
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        Some(expected.clone()),
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            integer_type,
            left.clone(),
            right.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        expected,
        "ordinary multiply dispatch selects the same-root quadratic before rectangles",
    );

    let bounds = |minimum, maximum| {
        let mut bounded = definitions.clone();
        bounded.extend([
            Proposition::LessOrEqual(literal(minimum), root.clone()),
            Proposition::LessOrEqual(root.clone(), literal(maximum)),
        ]);
        bounded
    };
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &bounds(16, 17),
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a wholly out-of-carrier quadratic range is falsehood",
    );
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &bounds(15, 16),
            definitions.len(),
            &parameters,
        ),
        None,
        "a partially overlapping quadratic range is not admitted",
    );
    let mut one_sided = definitions.clone();
    one_sided.push(lower.clone());
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &one_sided,
            definitions.len(),
            &parameters,
        ),
        None,
        "both direct signature endpoints are mandatory",
    );
    let mut relational_only = definitions.clone();
    relational_only.push(Proposition::LessOrEqual(
        root.clone(),
        ScalarTerm::exact_integer_subtract(
            integer_type,
            literal(i128::from(i8::MAX)),
            root.clone(),
        )
        .expect("MAX - root"),
    ));
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &relational_only,
            definitions.len(),
            &parameters,
        ),
        None,
        "relational premises do not replace unary signature bounds",
    );

    let other_root_id = ValueId::new(1814).expect("other root");
    let other_root = ScalarTerm::value(other_root_id, ScalarType::Integer(integer_type));
    let distinct_definitions = vec![
        definitions[0].clone(),
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, other_root, literal(10))
                .expect("other root - 10"),
        ),
    ];
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &distinct_definitions,
            distinct_definitions.len(),
            &BTreeSet::from([root_id, other_root_id]),
        ),
        None,
        "distinct roots remain on the rectangle family",
    );

    let zero = value(1815);
    let mut zero_definitions = vec![
        definitions[0].clone(),
        Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), literal(0))
                .expect("root * 0"),
        ),
    ];
    zero_definitions.extend([lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            zero,
            &zero_definitions,
            2,
            &parameters,
        ),
        None,
        "a zero branch is a narrower constant collapse, not a quadratic",
    );

    let mut reordered = vec![definitions[1].clone(), definitions[0].clone()];
    reordered.extend([lower, upper]);
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            &reordered,
            definitions.len(),
            &parameters,
        ),
        None,
        "the disjoint branch definitions remain source ordered",
    );
    assert_eq!(
        exact_integer_affine_quadratic_range(
            IntegerOffset::Nonnegative(i128::MAX as u128),
            IntegerOffset::Nonnegative(0),
            IntegerOffset::Nonnegative(2),
            IntegerOffset::Nonnegative(0),
            (-1, 1),
        ),
        None,
        "checked quadratic composition failure admits no family",
    );
    assert_eq!(
        exact_integer_same_root_affine_product_join_obligation(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
            left,
            right,
            &bounded,
            definitions.len(),
            &parameters,
        ),
        None,
        "the correlated quadratic family is signed-only",
    );
}

#[test]
fn same_root_affine_divide_remainder_join_excludes_exact_forbidden_lattice_roots() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let root_id = ValueId::new(1821).expect("divide/remainder root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("divide/remainder value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal =
        |value| ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal");
    let left_offset = value(1822);
    let left = value(1823);
    let right_product = value(1824);
    let right = value(1825);
    let definitions = vec![
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(64))
                .expect("root + 64"),
        ),
        Proposition::Equal(
            left.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset.clone(), literal(-2))
                .expect("left branch * -2"),
        ),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), literal(2))
                .expect("root * 2"),
        ),
        Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_add(integer_type, right_product.clone(), literal(1))
                .expect("right branch + 1"),
        ),
    ];
    let parameters = BTreeSet::from([root_id]);
    let context = PropositionContext::from_value_types((1821..=1829).map(|id| {
        (
            ValueId::new(id).expect("divide/remainder value"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("divide/remainder proposition context");
    let lower = Proposition::LessOrEqual(literal(-1), root.clone());
    let upper = Proposition::LessOrEqual(root.clone(), literal(0));
    let mut bounded = definitions.clone();
    bounded.extend([
        Proposition::LessOrEqual(literal(-10), root.clone()),
        lower.clone(),
        upper.clone(),
    ]);
    let expected = canonical_conjunction(vec![lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        Some(expected.clone()),
        "the odd divisor has no integer zero and MIN never coincides with divisor -1",
    );
    assert_eq!(
        exact_integer_divide_obligation_with_definitions(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        expected.clone(),
    );
    assert_eq!(
        exact_integer_remainder_obligation_with_definitions(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        expected,
    );

    let mut zero_definitions = definitions.clone();
    zero_definitions[3] = Proposition::Equal(
        right.clone(),
        ScalarTerm::exact_integer_add(integer_type, right_product, literal(0))
            .expect("right branch + 0"),
    );
    let mut partial_zero = zero_definitions.clone();
    partial_zero.extend([lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &partial_zero,
            zero_definitions.len(),
            &parameters,
        ),
        None,
        "one zero-divisor lattice point makes safety partial",
    );
    let mut all_zero = zero_definitions.clone();
    all_zero.extend([
        Proposition::LessOrEqual(literal(0), root.clone()),
        Proposition::LessOrEqual(root.clone(), literal(0)),
    ]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &all_zero,
            zero_definitions.len(),
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a singleton zero-divisor interval is wholly unsafe",
    );

    let coincident_offset = value(1826);
    let coincident_left = value(1827);
    let identity_right = value(1828);
    let coincident_definitions = vec![
        Proposition::Equal(
            coincident_offset.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(63))
                .expect("root - 63"),
        ),
        Proposition::Equal(
            coincident_left.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, coincident_offset, literal(2))
                .expect("coincident numerator"),
        ),
        Proposition::Equal(
            identity_right.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), literal(1))
                .expect("identity divisor"),
        ),
    ];
    let mut all_forbidden = coincident_definitions.clone();
    all_forbidden.extend([lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            coincident_left.clone(),
            identity_right,
            &all_forbidden,
            coincident_definitions.len(),
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "zero at x=0 and the MIN/-1 coincidence at x=-1 cover the whole interval",
    );

    let mut coincident_partial = definitions.clone();
    coincident_partial[0] = Proposition::Equal(
        left_offset,
        ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(63))
            .expect("root - 63"),
    );
    coincident_partial[1] = Proposition::Equal(
        left.clone(),
        ScalarTerm::exact_integer_multiply(integer_type, value(1822), literal(2))
            .expect("coincident left"),
    );
    let mut coincident_bounded = coincident_partial.clone();
    coincident_bounded.extend([lower.clone(), upper.clone()]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &coincident_bounded,
            coincident_partial.len(),
            &parameters,
        ),
        None,
        "one MIN/-1 coincidence also makes safety partial",
    );

    let mut one_sided = definitions.clone();
    one_sided.push(lower.clone());
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &one_sided,
            definitions.len(),
            &parameters,
        ),
        None,
        "both unary signature endpoints are mandatory",
    );
    let bounds_in_definition_carrier = bounded.clone();
    let all_axioms_are_definitions = bounds_in_definition_carrier.len();
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &bounds_in_definition_carrier,
            all_axioms_are_definitions,
            &parameters,
        ),
        None,
        "operation-definition axioms never manufacture signature authority",
    );
    let other_root_id = ValueId::new(1829).expect("other root");
    let other_root = ScalarTerm::value(other_root_id, ScalarType::Integer(integer_type));
    let mut distinct_definitions = definitions.clone();
    distinct_definitions[2] = Proposition::Equal(
        value(1824),
        ScalarTerm::exact_integer_multiply(integer_type, other_root, literal(2))
            .expect("other root * 2"),
    );
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            left.clone(),
            right.clone(),
            &distinct_definitions,
            distinct_definitions.len(),
            &BTreeSet::from([root_id, other_root_id]),
        ),
        None,
        "distinct roots remain fenced",
    );
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
            left,
            right,
            &bounded,
            definitions.len(),
            &parameters,
        ),
        None,
        "the exact forbidden-root family is signed-only",
    );
}

#[test]
fn affine_divide_remainder_checker_boundary_rejects_literal_and_bound_index_drift() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("checker-boundary value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value))
            .expect("checker-boundary literal")
    };
    let root_id = ValueId::new(1901).expect("checker-boundary root");
    let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
    let sixty_four = value(1902);
    let left_offset = value(1903);
    let negative_two = value(1904);
    let dividend = value(1905);
    let two = value(1906);
    let right_product = value(1907);
    let divisor = value(1908);
    let loose_lower = Proposition::LessOrEqual(literal(-10), root.clone());
    let lower = Proposition::LessOrEqual(literal(-1), root.clone());
    let loose_upper = Proposition::LessOrEqual(root.clone(), literal(10));
    let upper = Proposition::LessOrEqual(root.clone(), literal(0));
    let axioms = vec![
        Proposition::Equal(sixty_four.clone(), literal(64)),
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), sixty_four)
                .expect("root + landed 64"),
        ),
        Proposition::Equal(negative_two.clone(), literal(-2)),
        Proposition::Equal(
            dividend.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset, negative_two)
                .expect("dividend branch"),
        ),
        Proposition::Equal(two.clone(), literal(2)),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), two)
                .expect("divisor product"),
        ),
        Proposition::Equal(
            divisor.clone(),
            ScalarTerm::exact_integer_add(integer_type, right_product, literal(1))
                .expect("odd divisor"),
        ),
        loose_lower,
        lower.clone(),
        loose_upper,
        upper.clone(),
    ];
    let context = PropositionContext::from_value_types((1901..=1908).map(|id| {
        (
            ValueId::new(id).expect("checker-boundary value"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("checker-boundary proposition context");
    let parameters = BTreeSet::from([root_id]);
    let expected = canonical_conjunction(vec![lower, upper]);
    assert_eq!(
        exact_integer_same_root_affine_divide_remainder_join_obligation(
            &context,
            integer_type,
            dividend.clone(),
            divisor.clone(),
            &axioms,
            7,
            &parameters,
        ),
        Some(expected.clone()),
        "the reducer retains exact prior literal and tight-bound coordinates for the checker",
    );

    let witness = IntegerCorrelatedForbiddenRootWitness {
        dividend: CorrelatedAffineBranchWitness {
            root: root.clone(),
            target: dividend,
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 1,
                    literal_axiom: Some(0),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 3,
                    literal_axiom: Some(2),
                },
            ],
        },
        divisor: CorrelatedAffineBranchWitness {
            root,
            target: divisor,
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 5,
                    literal_axiom: Some(4),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 6,
                    literal_axiom: None,
                },
            ],
        },
        definition_axiom_count: 7,
        lower_bound_axiom: 8,
        upper_bound_axiom: 10,
        conclusion: expected,
    };
    let check = |candidate: &IntegerCorrelatedForbiddenRootWitness| {
        check_integer_correlated_forbidden_root_witness(&context, &axioms, &parameters, candidate)
    };
    check(&witness).expect("exact retained coordinates check");

    let mut late_literal = witness.clone();
    late_literal.dividend.steps[0].literal_axiom = Some(1);
    assert_eq!(
        check(&late_literal),
        Err(
            IntegerCorrelatedForbiddenRootWitnessError::LiteralAxiomNotPrior {
                definition_axiom: 1,
                literal_axiom: 1,
            },
        ),
    );
    let mut missing_literal = witness.clone();
    missing_literal.dividend.steps[0].literal_axiom = None;
    assert_eq!(
        check(&missing_literal),
        Err(IntegerCorrelatedForbiddenRootWitnessError::MissingLiteralAxiom(1)),
    );
    let mut reused_literal = witness.clone();
    reused_literal.dividend.steps[1].literal_axiom = Some(0);
    assert_eq!(
        check(&reused_literal),
        Err(IntegerCorrelatedForbiddenRootWitnessError::LiteralIdentityMismatch(3)),
    );
    let mut lower_drift = witness.clone();
    lower_drift.lower_bound_axiom = 7;
    assert_eq!(
        check(&lower_drift),
        Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch),
    );
    let mut upper_drift = witness;
    upper_drift.upper_bound_axiom = 9;
    assert_eq!(
        check(&upper_drift),
        Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch),
    );
}

#[test]
fn distinct_root_affine_product_join_uses_the_exact_four_corner_hull() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
    let left_root_id = ValueId::new(1821).expect("left root");
    let right_root_id = ValueId::new(1822).expect("right root");
    let left_root = ScalarTerm::value(left_root_id, ScalarType::Integer(integer_type));
    let right_root = ScalarTerm::value(right_root_id, ScalarType::Integer(integer_type));
    let value = |id| {
        ScalarTerm::value(
            ValueId::new(id).expect("product value"),
            ScalarType::Integer(integer_type),
        )
    };
    let literal =
        |value| ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal");
    let left_offset = value(1823);
    let left_product = value(1824);
    let right_offset = value(1825);
    let right_product = value(1826);
    let definitions = vec![
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, left_root.clone(), literal(1))
                .expect("left + 1"),
        ),
        Proposition::Equal(
            left_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset, literal(2))
                .expect("left branch * 2"),
        ),
        Proposition::Equal(
            right_offset.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, right_root.clone(), literal(1))
                .expect("right - 1"),
        ),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, right_offset, literal(3))
                .expect("right branch * 3"),
        ),
    ];
    let parameters = BTreeSet::from([left_root_id, right_root_id]);
    let left_lower = Proposition::LessOrEqual(literal(-10), left_root.clone());
    let left_upper = Proposition::LessOrEqual(left_root.clone(), literal(10));
    let right_lower = Proposition::LessOrEqual(literal(-10), right_root.clone());
    let right_upper = Proposition::LessOrEqual(right_root.clone(), literal(10));
    let mut bounded = definitions.clone();
    bounded.extend([
        Proposition::LessOrEqual(literal(-100), left_root.clone()),
        left_lower.clone(),
        left_upper.clone(),
        right_lower.clone(),
        right_upper.clone(),
    ]);
    let expected = canonical_conjunction(vec![
        left_lower.clone(),
        left_upper.clone(),
        right_lower.clone(),
        right_upper.clone(),
    ]);
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        Some(expected.clone()),
        "the mixed-sign four-corner hull lies wholly inside i16",
    );
    assert_eq!(
        exact_integer_multiply_obligation_with_definitions(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &bounded,
            definitions.len(),
            &parameters,
        ),
        expected,
        "ordinary multiply dispatch selects the product rectangle",
    );

    let bounds = |minimum, maximum| {
        let mut bounded = definitions.clone();
        bounded.extend([
            Proposition::LessOrEqual(literal(minimum), left_root.clone()),
            Proposition::LessOrEqual(left_root.clone(), literal(maximum)),
            Proposition::LessOrEqual(literal(minimum), right_root.clone()),
            Proposition::LessOrEqual(right_root.clone(), literal(maximum)),
        ]);
        bounded
    };
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &bounds(100, 101),
            definitions.len(),
            &parameters,
        ),
        Some(Proposition::Falsehood),
        "a wholly out-of-carrier positive product is falsehood",
    );
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &bounds(50, 100),
            definitions.len(),
            &parameters,
        ),
        None,
        "a partially overlapping product hull is not admitted",
    );
    let mut one_sided = definitions.clone();
    one_sided.extend([left_lower.clone(), left_upper.clone(), right_lower.clone()]);
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &one_sided,
            definitions.len(),
            &parameters,
        ),
        None,
        "both unary endpoints are mandatory for both roots",
    );
    let relational = Proposition::LessOrEqual(
        left_root.clone(),
        ScalarTerm::exact_integer_subtract(
            integer_type,
            literal(i128::from(i16::MAX)),
            right_root.clone(),
        )
        .expect("MAX - right"),
    );
    let mut relational_only = definitions.clone();
    relational_only.push(relational);
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            right_product.clone(),
            &relational_only,
            definitions.len(),
            &parameters,
        ),
        None,
    );
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left_product.clone(),
            left_product,
            &bounded,
            definitions.len(),
            &parameters,
        ),
        None,
        "same-root multiplication remains a correlated quadratic fence",
    );
    let mut reordered = vec![
        definitions[2].clone(),
        definitions[3].clone(),
        definitions[0].clone(),
        definitions[1].clone(),
    ];
    reordered.extend([
        left_lower.clone(),
        left_upper.clone(),
        right_lower.clone(),
        right_upper.clone(),
    ]);
    assert_eq!(
        exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            value(1824),
            value(1826),
            &reordered,
            definitions.len(),
            &parameters,
        ),
        None,
        "the two branch walks remain source ordered",
    );
}
