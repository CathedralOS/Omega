use super::indexes::{
    SelectorLocation, SelectorSnapshotEvaluation,
    index_expression_may_contain_fixed_with_selectors,
    index_expression_may_overlap_fixed_range_with_selectors,
    index_expressions_may_overlap_with_selectors,
};
use crate::flow::place_segment_has_unresolved_identity;
use checked_trees::{
    BorrowCompatibilityPlaceSide, BorrowCompatibilitySelectorSnapshot, CapturedPlaceContainment,
};

pub(super) fn place_segments_containment(
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> CapturedPlaceContainment {
    if left
        .iter()
        .chain(right)
        .any(|segment| place_segment_has_unresolved_identity(*segment))
    {
        return CapturedPlaceContainment::None;
    }
    if left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| structural_segment_equal(*left, *right))
    {
        return CapturedPlaceContainment::Same;
    }
    if path_contains(left, right) {
        return CapturedPlaceContainment::LeftContainsRight;
    }
    if path_contains(right, left) {
        return CapturedPlaceContainment::RightContainsLeft;
    }
    CapturedPlaceContainment::None
}

fn path_contains(container: &[facts::PlaceSegment], contained: &[facts::PlaceSegment]) -> bool {
    container.len() <= contained.len()
        && container
            .iter()
            .zip(contained)
            .all(|(container, contained)| segment_contains(*container, *contained))
}

fn segment_contains(container: facts::PlaceSegment, contained: facts::PlaceSegment) -> bool {
    if structural_segment_equal(container, contained) {
        return true;
    }
    match (container, contained) {
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::FixedIndex { index },
        ) => start < end && start <= index && index < end,
        (
            facts::PlaceSegment::FixedRange {
                start: outer_start,
                end: outer_end,
            },
            facts::PlaceSegment::FixedRange {
                start: inner_start,
                end: inner_end,
            },
        ) => {
            outer_start < outer_end
                && inner_start < inner_end
                && outer_start <= inner_start
                && inner_end <= outer_end
        }
        _ => false,
    }
}

fn structural_segment_equal(left: facts::PlaceSegment, right: facts::PlaceSegment) -> bool {
    match (left, right) {
        (
            facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            facts::PlaceSegment::Case {
                variant: left_variant,
            },
            facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            facts::PlaceSegment::FixedIndex { index: left_index },
            facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            facts::PlaceSegment::FixedRange {
                start: left_start,
                end: left_end,
            },
            facts::PlaceSegment::FixedRange {
                start: right_start,
                end: right_end,
            },
        ) => left_start == right_start && left_end == right_end,
        // Runtime/symbolic selectors need a captured value/version before they
        // can establish containment, even when expression handles happen to
        // match. The existing overlap tactic remains conservative for them.
        _ => false,
    }
}

#[cfg(test)]
pub(super) fn place_segments_may_overlap(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let mut selectors = SelectorSnapshotEvaluation::capture();
    place_segments_may_overlap_evaluated(program, left, right, &mut selectors)
}

pub(super) fn place_segments_may_overlap_with_snapshot(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> (bool, Vec<BorrowCompatibilitySelectorSnapshot>) {
    let mut selectors = SelectorSnapshotEvaluation::capture();
    let may_overlap = place_segments_may_overlap_evaluated(program, left, right, &mut selectors);
    (
        may_overlap,
        selectors
            .finish()
            .expect("captured selector evaluation is always complete"),
    )
}

pub(super) fn place_segments_may_overlap_from_snapshot(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    snapshot: &[BorrowCompatibilitySelectorSnapshot],
) -> Option<bool> {
    let mut selectors = SelectorSnapshotEvaluation::replay(snapshot);
    let may_overlap = place_segments_may_overlap_evaluated(program, left, right, &mut selectors);
    selectors.finish().map(|_| may_overlap)
}

fn place_segments_may_overlap_evaluated(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    for (segment_index, (&left_segment, &right_segment)) in left.iter().zip(right).enumerate() {
        // A known prefix may already have proved disjointness. Once nominal
        // identity is missing, later children cannot establish a divergence.
        if place_segment_has_unresolved_identity(left_segment)
            || place_segment_has_unresolved_identity(right_segment)
        {
            return true;
        }
        if !place_segment_pair_may_overlap(
            program,
            left_segment,
            right_segment,
            segment_index,
            selectors,
        ) {
            return false;
        }
    }
    true
}

fn place_segment_pair_may_overlap(
    program: &typed_trees::TypedTrees,
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
    segment_index: usize,
    selectors: &mut SelectorSnapshotEvaluation<'_>,
) -> bool {
    let left_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Forming,
        segment_index,
    };
    let right_location = SelectorLocation {
        side: BorrowCompatibilityPlaceSide::Active,
        segment_index,
    };
    match (left, right) {
        (
            facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            facts::PlaceSegment::Case {
                variant: left_variant,
            },
            facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            facts::PlaceSegment::FixedIndex { index: left_index },
            facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            facts::PlaceSegment::FixedRange {
                start: left_start,
                end: left_end,
            },
            facts::PlaceSegment::FixedRange {
                start: right_start,
                end: right_end,
            },
        ) => {
            left_start < left_end
                && right_start < right_end
                && left_start < right_end
                && right_start < left_end
        }
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::FixedIndex { index },
        )
        | (
            facts::PlaceSegment::FixedIndex { index },
            facts::PlaceSegment::FixedRange { start, end },
        ) => start < end && start <= index && index < end,
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::Index { expression },
        ) => index_expression_may_overlap_fixed_range_with_selectors(
            program,
            expression,
            right_location,
            start,
            end,
            selectors,
        ),
        (
            facts::PlaceSegment::Index { expression },
            facts::PlaceSegment::FixedRange { start, end },
        ) => index_expression_may_overlap_fixed_range_with_selectors(
            program,
            expression,
            left_location,
            start,
            end,
            selectors,
        ),
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression }) => {
            index_expression_may_contain_fixed_with_selectors(
                program,
                expression,
                right_location,
                index,
                selectors,
            )
        }
        (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            index_expression_may_contain_fixed_with_selectors(
                program,
                expression,
                left_location,
                index,
                selectors,
            )
        }
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap_with_selectors(
            program,
            left_expression,
            left_location,
            right_expression,
            right_location,
            selectors,
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    };

    fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    #[test]
    fn unresolved_field_does_not_prove_disjointness() {
        let program = typed_trees::TypedTrees::default();
        let unresolved = facts::PlaceSegment::default();
        let resolved = facts::PlaceSegment::Field {
            symbol: symbols::SymbolHandle::from_arena_index(1),
        };
        assert!(place_segments_may_overlap(
            &program,
            &[unresolved],
            &[resolved]
        ));
    }

    #[test]
    fn unresolved_field_does_not_prove_containment() {
        let unresolved = facts::PlaceSegment::default();
        assert_eq!(
            place_segments_containment(&[unresolved], &[unresolved]),
            CapturedPlaceContainment::None
        );
    }

    #[test]
    fn fixed_indices_overlap_only_the_same_element() {
        let program = typed_trees::TypedTrees::default();

        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::FixedIndex { index: 1 }],
        ));
    }

    #[test]
    fn fixed_and_legacy_literal_indices_compare_by_value() {
        let mut program = typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: zero }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn fixed_range_preserves_pure_constant_index_folding() {
        let mut program = typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let two = integer_expression(&mut program, 2);
        let three =
            program
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
    fn fixed_index_before_tail_range_is_disjoint() {
        let mut program = typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let tail = program.expression_table.insert(ExpressionNode::Range(
            typed_trees::expression::TableRangeExpression {
                start: one,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            },
        ));

        assert!(!place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 0 }],
            &[facts::PlaceSegment::Index { expression: tail }],
        ));
        assert!(place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::FixedIndex { index: 1 }],
            &[facts::PlaceSegment::Index { expression: tail }],
        ));
    }

    #[test]
    fn fixed_ranges_use_half_open_overlap() {
        let program = typed_trees::TypedTrees::default();
        let range = |start, end| facts::PlaceSegment::FixedRange { start, end };

        assert!(place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(1, 3)],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(2, 4)],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[range(1, 1)],
            &[range(0, 2)],
        ));
    }

    #[test]
    fn fixed_structural_containment_is_directional() {
        let range = |start, end| facts::PlaceSegment::FixedRange { start, end };
        let index = |index| facts::PlaceSegment::FixedIndex { index };

        assert_eq!(
            place_segments_containment(&[range(0, 4)], &[index(2)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&[index(2)], &[range(0, 4)]),
            CapturedPlaceContainment::RightContainsLeft
        );
        assert_eq!(
            place_segments_containment(&[range(0, 8)], &[range(2, 4)]),
            CapturedPlaceContainment::LeftContainsRight
        );
        assert_eq!(
            place_segments_containment(&[range(0, 4)], &[range(4, 8)]),
            CapturedPlaceContainment::None
        );
    }
}
