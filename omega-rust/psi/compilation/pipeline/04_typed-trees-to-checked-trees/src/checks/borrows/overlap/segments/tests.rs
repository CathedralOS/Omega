//! Segment overlap tests.

use super::CapturedPlaceContainment;
use crate::checks::borrows::overlap::segments::place_segments_containment;
use crate::checks::borrows::overlap::segments::place_segments_may_overlap;
use checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::expression::TableRangeExpression;

fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
    program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(value),
    ))
}

fn range_bounds(
    program: &mut typed_trees::TypedTrees,
    start: ExpressionHandle,
    end: ExpressionHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive: false,
        }))
}

#[test]
fn fixed_range_preserves_pure_constant_index_folding() {
    let mut program = typed_trees::TypedTrees::default();
    let one = integer_expression(&mut program, 1);
    let two = integer_expression(&mut program, 2);
    let three = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: two,
        }));

    assert!(!place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedRange { start: 0, end: 3 }],
        &[facts::PlaceSegment::Index { expression: three }],
    ));
    assert!(place_segments_may_overlap(
        &program,
        &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
        &[facts::PlaceSegment::Index { expression: three }],
    ));
}

#[test]
fn literal_window_expressions_prove_containment_and_equality() {
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer_expression(&mut program, 0);
    let one = integer_expression(&mut program, 1);
    let three = integer_expression(&mut program, 3);
    let four = integer_expression(&mut program, 4);
    let outer = range_bounds(&mut program, zero, four);
    let inner = range_bounds(&mut program, one, three);
    let index = |expression| facts::PlaceSegment::Index { expression };

    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(inner)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(three)]),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(&program, &[index(outer)], &[index(four)]),
        CapturedPlaceContainment::None
    );
    // Literal windows inside `FixedRange` selectors compare by value too.
    assert_eq!(
        place_segments_containment(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
            &[index(inner)]
        ),
        CapturedPlaceContainment::LeftContainsRight
    );
    assert_eq!(
        place_segments_containment(
            &program,
            &[facts::PlaceSegment::FixedRange { start: 0, end: 4 }],
            &[index(three)]
        ),
        CapturedPlaceContainment::LeftContainsRight
    );
}
