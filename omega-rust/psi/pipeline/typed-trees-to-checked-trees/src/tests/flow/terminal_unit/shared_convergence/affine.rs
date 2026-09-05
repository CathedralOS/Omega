//! Affine chain and correlated-join classifier matrices.

use super::*;

#[test]
fn exact_affine_chain_classifier_accepts_only_left_associated_landed_mixed_operations() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U8,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        parameter(0),
        literal(3i64),
    );
    let multiplied = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        added.clone(),
        literal(2i64),
    );
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        multiplied,
        literal(1i64),
    );
    assert_eq!(
        exact_affine_chain_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0])
    );
    for invalid in [
        added,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            parameter(0),
            literal(2i64),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(0),
                literal(1i64),
            ),
            parameter(0),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            literal(2i64),
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(0),
                literal(1i64),
            ),
        ),
    ] {
        assert_eq!(
            exact_affine_chain_runtime_parameter_positions_for_test(&invalid, 1),
            None
        );
    }
}

#[test]
fn affine_chain_exact_cast_classifier_reuses_only_the_unified_mixed_family() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U16,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let parameter = || CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U16,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        parameter(),
        literal(3i64),
    );
    let affine = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            added.clone(),
            literal(2i64),
        ),
        literal(1i64),
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &affine, 1,),
        Some(vec![0]),
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(PrimitiveType::U8, &added, 1,),
        None,
        "homogeneous chains remain on the narrower computed-cast classifiers",
    );
    assert_eq!(
        exact_affine_chain_cast_runtime_parameter_positions_for_test(
            PrimitiveType::Addr,
            &affine,
            1,
        ),
        None,
    );
}

#[test]
fn exact_cast_then_affine_classifier_accepts_only_the_unified_mixed_family() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let cast = || CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    };
    let added = operation(CheckedIntegerBinaryKind::ExactAdd, cast(), literal(3i64));
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            added.clone(),
            literal(2i64),
        ),
        literal(1i64),
    );
    assert_eq!(
        exact_cast_then_affine_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );
    for invalid in [
        added,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            cast(),
            literal(2i64),
        ),
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(CheckedIntegerBinaryKind::ExactAdd, cast(), literal(1i64)),
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U8,
            },
        ),
    ] {
        assert_eq!(
            exact_cast_then_affine_runtime_parameter_positions_for_test(&invalid, 1),
            None,
        );
    }
}

#[test]
fn exact_affine_cast_affine_classifier_unifies_both_nonempty_sides() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U16,
            },
            literal(3i64, numerics::literals::LandedIntegerType::U16),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U16),
    );
    let cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(source),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let accepted = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U8,
        operation(
            CheckedIntegerBinaryKind::ExactSubtract,
            PrimitiveType::U8,
            cast,
            literal(1i64, numerics::literals::LandedIntegerType::U8),
        ),
        literal(2i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&accepted, 1),
        Some(vec![0]),
    );

    let direct_cast = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U16,
        }),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let empty_source_side = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::U8,
        direct_cast,
        literal(1i64, numerics::literals::LandedIntegerType::U8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&empty_source_side, 1),
        None,
    );

    let negative_source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I16,
        },
        literal(-1i64, numerics::literals::LandedIntegerType::I16),
    );
    let fenced = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        CheckedScalarExpression::IntegerExactCast {
            primitive_type: PrimitiveType::I8,
            operand: Box::new(negative_source),
            range: checked_trees::CheckedIntegerRange::default(),
        },
        literal(1i64, numerics::literals::LandedIntegerType::I8),
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&fenced, 1),
        None,
    );
}

#[test]
fn signed_affine_cast_affine_classifier_preserves_two_branch_priority() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |primitive_type| CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type,
    };
    let i16_literal = |value| literal(value, numerics::literals::LandedIntegerType::I16);
    let i8_literal = |value| literal(value, numerics::literals::LandedIntegerType::I8);
    let cast = |source| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::I8,
        operand: Box::new(source),
        range: checked_trees::CheckedIntegerRange::default(),
    };
    let source = |factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(PrimitiveType::I16),
                i16_literal(3),
            ),
            i16_literal(factor),
        )
    };
    let target = |root, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I8,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I8,
                root,
                i8_literal(3),
            ),
            i8_literal(factor),
        )
    };

    let source_negative = target(cast(source(-2)), 2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&source_negative, 1,),
        Some(vec![0]),
    );
    let target_negative = target(cast(source(2)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&target_negative, 1,),
        Some(vec![0]),
    );
    let both_negative = target(cast(source(-2)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&both_negative, 1),
        Some(vec![0]),
    );

    let all_nonnegative = target(cast(source(2)), 2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&all_nonnegative, 1,),
        None,
        "the established nonnegative sandwich keeps priority",
    );
    assert_eq!(
        exact_affine_cast_affine_runtime_parameter_positions_for_test(&all_nonnegative, 1),
        Some(vec![0]),
    );
    let empty_source = target(cast(parameter(PrimitiveType::I16)), -2);
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&empty_source, 1),
        None,
        "the one-sided post-cast signed-affine path keeps priority",
    );
    let source_product_without_offset = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        parameter(PrimitiveType::I16),
        i16_literal(-2),
    );
    let target_offset_only = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        cast(source_product_without_offset),
        i8_literal(1),
    );
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(
            &target_offset_only,
            1,
        ),
        None,
        "a thin homogeneous-product/offset permutation remains fenced",
    );

    let i64_literal = |value| literal(value, numerics::literals::LandedIntegerType::I64);
    let overflow_source = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                operation(
                    CheckedIntegerBinaryKind::ExactAdd,
                    PrimitiveType::I64,
                    parameter(PrimitiveType::I64),
                    i64_literal(0),
                ),
                i64_literal(i64::MIN),
            ),
            i64_literal(i64::MIN),
        ),
        i64_literal(4),
    );
    let overflow = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I8,
        cast(overflow_source),
        i8_literal(1),
    );
    assert_eq!(
        exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(&overflow, 1),
        None,
        "checked coefficient overflow admits no shared runtime-input family",
    );
}

#[test]
fn affine_fork_join_classifier_requires_two_disjoint_branches_on_one_root() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let i16_literal = |value| literal(value, numerics::literals::LandedIntegerType::I16);
    let branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                i16_literal(offset),
            ),
            i16_literal(factor),
        )
    };

    let joined = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        branch(0, -1, 3),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0]),
    );
    let cancellation = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        PrimitiveType::I16,
        branch(0, 3, -2),
        branch(0, -4, -2),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&cancellation, 2),
        Some(vec![0]),
        "a zero combined coefficient remains a valid join-local result",
    );

    let distinct_roots = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        branch(1, -1, 3),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&distinct_roots, 2),
        None,
        "distinct roots require multivariate proof design",
    );
    let empty_right = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        parameter(0, PrimitiveType::I16),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&empty_right, 2),
        None,
        "both proof-bearing branches must be nonempty",
    );
    let runtime_sibling = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(0, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
        branch(0, 1, 2),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&runtime_sibling, 2),
        None,
        "a branch sibling must be an independently landed literal",
    );
    let linear_chain = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        branch(0, 1, 2),
        i16_literal(1),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&linear_chain, 2),
        None,
        "one-sided chains retain their existing classifier priority",
    );

    let i64_literal = |value| literal(value, numerics::literals::LandedIntegerType::I64);
    let overflow_branch = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I64,
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I64,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I64,
                parameter(0, PrimitiveType::I64),
                i64_literal(i64::MAX),
            ),
            i64_literal(i64::MAX),
        ),
        i64_literal(i64::MAX),
    );
    let overflow = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I64,
        overflow_branch,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I64,
            parameter(0, PrimitiveType::I64),
            i64_literal(1),
        ),
    );
    assert_eq!(
        exact_affine_fork_join_runtime_parameter_positions_for_test(&overflow, 2),
        None,
        "checked branch coefficient overflow admits no family",
    );
}

#[test]
fn distinct_root_affine_fork_join_classifier_requires_two_direct_roots() {
    let literal = |value| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::I16,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::I16,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::I16,
    };
    let branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                parameter(position),
                literal(offset),
            ),
            literal(factor),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        branch(1, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0, 1]),
    );
    let signed_subtract = operation(
        CheckedIntegerBinaryKind::ExactSubtract,
        branch(0, 3, -2),
        branch(1, -4, -2),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(
            &signed_subtract,
            2,
        ),
        Some(vec![0, 1]),
    );
    let same_root = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        branch(0, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&same_root, 2),
        None,
        "the correlated same-root family retains dispatch priority",
    );
    let empty_branch = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        parameter(1),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(&empty_branch, 2,),
        None,
    );
    let computed_sibling = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        branch(0, 1, 2),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            parameter(1),
            parameter(0),
        ),
    );
    assert_eq!(
        exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(
            &computed_sibling,
            2,
        ),
        None,
        "every branch edge requires an independently landed literal sibling",
    );
}

#[test]
fn distinct_root_affine_product_join_classifier_requires_signed_direct_roots() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let signed_branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, numerics::literals::LandedIntegerType::I16),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(1, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&joined, 2),
        Some(vec![0, 1]),
    );
    let negative = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 3, -2),
        signed_branch(1, -4, -2),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&negative, 2),
        Some(vec![0, 1]),
        "negative affine coefficients reverse their own forward endpoints",
    );
    let same_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 3),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&same_root, 2,),
        None,
        "same-root multiplication requires correlated quadratic algebra",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        parameter(1, PrimitiveType::I16),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(
            &direct_side,
            2,
        ),
        None,
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(1, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(
            &computed_root,
            2,
        ),
        None,
    );
    let unsigned_branch = |position| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(position, PrimitiveType::U16),
            literal(1, numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        unsigned_branch(0),
        unsigned_branch(1),
    );
    assert_eq!(
        exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(&unsigned, 2),
        None,
        "the initial product-rectangle family is signed-only",
    );
}

#[test]
fn same_root_affine_product_join_classifier_requires_a_genuine_signed_quadratic() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let signed_branch = |position, offset, factor| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, numerics::literals::LandedIntegerType::I16),
        )
    };
    let joined = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 3),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&joined, 1),
        Some(vec![0]),
    );
    let concave = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 3, -2),
        signed_branch(0, -4, 2),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&concave, 1),
        Some(vec![0]),
        "opposite branch signs retain a genuine concave quadratic",
    );
    let distinct_roots = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(1, -1, 3),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(
            &distinct_roots,
            2,
        ),
        None,
        "distinct roots remain on the independent rectangle path",
    );
    let zero_branch = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        signed_branch(0, -1, 0),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&zero_branch, 1),
        None,
        "a constant collapse is not a quadratic family",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        parameter(0, PrimitiveType::I16),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&direct_side, 1),
        None,
    );
    let computed_branch = operation(
        CheckedIntegerBinaryKind::ExactAdd,
        PrimitiveType::I16,
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            parameter(0, PrimitiveType::I16),
            parameter(0, PrimitiveType::I16),
        ),
        literal(1, numerics::literals::LandedIntegerType::I16),
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::I16,
        signed_branch(0, 1, 2),
        computed_branch,
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&computed_root, 1,),
        None,
        "a computed root is not signature-bound authority",
    );
    let unsigned_branch = |offset| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(offset, numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactMultiply,
        PrimitiveType::U16,
        unsigned_branch(1),
        unsigned_branch(2),
    );
    assert_eq!(
        exact_same_root_affine_product_join_runtime_parameter_positions_for_test(&unsigned, 1),
        None,
        "the initial correlated quadratic family is signed-only",
    );
}

#[test]
fn same_root_affine_divide_remainder_classifier_requires_two_genuine_signed_branches() {
    let literal = |value, landed_type| CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(value).with_landing(
            numerics::literals::IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            },
        ),
    };
    let operation = |kind, primitive_type, left, right| CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type,
        left: Box::new(left),
        right: Box::new(right),
    };
    let parameter = |position, primitive_type| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let branch = |position, offset, factor, offset_kind| {
        operation(
            CheckedIntegerBinaryKind::ExactMultiply,
            PrimitiveType::I16,
            operation(
                offset_kind,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(offset, numerics::literals::LandedIntegerType::I16),
            ),
            literal(factor, numerics::literals::LandedIntegerType::I16),
        )
    };
    let divisor = |position| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactMultiply,
                PrimitiveType::I16,
                parameter(position, PrimitiveType::I16),
                literal(2, numerics::literals::LandedIntegerType::I16),
            ),
            literal(1, numerics::literals::LandedIntegerType::I16),
        )
    };
    for kind in [
        CheckedIntegerBinaryKind::ExactDivide,
        CheckedIntegerBinaryKind::ExactRemainder,
    ] {
        let joined = operation(
            kind,
            PrimitiveType::I16,
            branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
            divisor(0),
        );
        assert_eq!(
            exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
                &joined, 1,
            ),
            Some(vec![0]),
        );
    }
    let distinct = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
        divisor(1),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &distinct, 2,
        ),
        None,
        "distinct roots cannot borrow correlation",
    );
    let zero_branch = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, 0, CheckedIntegerBinaryKind::ExactAdd),
        divisor(0),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &zero_branch,
            1,
        ),
        None,
        "constant collapse stays on narrower paths",
    );
    let direct_side = operation(
        CheckedIntegerBinaryKind::ExactRemainder,
        PrimitiveType::I16,
        parameter(0, PrimitiveType::I16),
        divisor(0),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &direct_side,
            1,
        ),
        None,
    );
    let computed_root = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::I16,
        branch(0, 16384, -2, CheckedIntegerBinaryKind::ExactAdd),
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::I16,
            operation(
                CheckedIntegerBinaryKind::ExactAdd,
                PrimitiveType::I16,
                parameter(0, PrimitiveType::I16),
                parameter(0, PrimitiveType::I16),
            ),
            literal(1, numerics::literals::LandedIntegerType::I16),
        ),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &computed_root,
            1,
        ),
        None,
        "computed roots are not signature-bound authority",
    );
    let unsigned_branch = |offset| {
        operation(
            CheckedIntegerBinaryKind::ExactAdd,
            PrimitiveType::U16,
            parameter(0, PrimitiveType::U16),
            literal(offset, numerics::literals::LandedIntegerType::U16),
        )
    };
    let unsigned = operation(
        CheckedIntegerBinaryKind::ExactDivide,
        PrimitiveType::U16,
        unsigned_branch(1),
        unsigned_branch(2),
    );
    assert_eq!(
        exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
            &unsigned, 1,
        ),
        None,
        "the lattice-correlation family is signed-only",
    );
}
