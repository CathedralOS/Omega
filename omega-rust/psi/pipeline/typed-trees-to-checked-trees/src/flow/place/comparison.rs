pub(crate) fn canonical_place_segments_equal(
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
) -> bool {
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
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => left_expression == right_expression,
        _ => false,
    }
}

use checked_trees::expression::ExpressionHandle;

#[allow(dead_code)]
pub(crate) fn canonical_place_overlaps_segments(
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            canonical_place_segments_equal(*left_segment, *right_segment)
        })
}

#[allow(dead_code)]
pub(crate) fn canonical_place_overlaps_joined_segments(
    prefix: &[facts::PlaceSegment],
    suffix: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let shared_len = prefix.len().saturating_add(suffix.len()).min(right.len());

    (0..shared_len).all(|index| {
        let left_segment = if index < prefix.len() {
            prefix[index]
        } else {
            suffix[index - prefix.len()]
        };
        canonical_place_segments_equal(left_segment, right[index])
    })
}

pub(crate) fn canonical_place_segments_may_overlap(
    program: &typed_trees::TypedTrees,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            canonical_place_segment_pair_may_overlap(program, *left_segment, *right_segment)
        })
}

pub(crate) fn canonical_place_joined_segments_may_overlap(
    program: &typed_trees::TypedTrees,
    prefix: &[facts::PlaceSegment],
    suffix: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let shared_len = prefix.len().saturating_add(suffix.len()).min(right.len());

    (0..shared_len).all(|index| {
        let left_segment = if index < prefix.len() {
            prefix[index]
        } else {
            suffix[index - prefix.len()]
        };
        canonical_place_segment_pair_may_overlap(program, left_segment, right[index])
    })
}

fn canonical_place_segment_pair_may_overlap(
    program: &typed_trees::TypedTrees,
    left: facts::PlaceSegment,
    right: facts::PlaceSegment,
) -> bool {
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
        ) => fixed_ranges_overlap(left_start, left_end, right_start, right_end),
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::FixedIndex { index },
        )
        | (
            facts::PlaceSegment::FixedIndex { index },
            facts::PlaceSegment::FixedRange { start, end },
        ) => fixed_range_contains(start, end, index),
        (
            facts::PlaceSegment::FixedRange { start, end },
            facts::PlaceSegment::Index { expression },
        )
        | (
            facts::PlaceSegment::Index { expression },
            facts::PlaceSegment::FixedRange { start, end },
        ) => expression_static_index(program, expression)
            .is_none_or(|index| fixed_range_contains(start, end, index)),
        (facts::PlaceSegment::FixedIndex { index }, facts::PlaceSegment::Index { expression })
        | (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            expression_static_index(program, expression).is_none_or(|value| value == index)
        }
        (
            facts::PlaceSegment::Index {
                expression: left_expression,
            },
            facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap(program, left_expression, right_expression),
        _ => false,
    }
}

fn fixed_range_contains(start: usize, end: usize, index: usize) -> bool {
    start < end && start <= index && index < end
}

fn fixed_ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < left_end
        && right_start < right_end
        && left_start < right_end
        && right_start < left_end
}

fn expression_static_index(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<usize> {
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
}

/// Decide whether two index expressions may select the same element.
///
/// Soundness requires defaulting to `true` (overlap) whenever we cannot prove the
/// indices are distinct, so a genuinely-overlapping or unknown-index mutation always
/// invalidates a dependent domain fact. We can only prove *disjointness* when both
/// sides are literal integers with different values; e.g. a domain fact over
/// `self.entries[0]` survives a mutation of `self.entries[1]`.
///
/// Dynamic indices (including repeated `self.index` reads that produce distinct
/// expression handles) are treated conservatively as possibly-overlapping. Disjoint
/// dynamic-indexed proofs are instead preserved by trailing-segment divergence — a
/// mutation of a different field of the same indexed element does not overlap the
/// fact's dependency path — which the joined-segment matcher handles independently of
/// this index comparison.
fn index_expressions_may_overlap(
    program: &typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (
            checked_trees::expression::ExpressionNode::Integer(left_value),
            checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => left_value == right_value,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
        program
            .expression_table
            .insert(checked_trees::expression::ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(value),
            ))
    }

    #[test]
    fn indexed_segment_overlap_uses_literal_values_not_handle_identity() {
        let mut program = typed_trees::TypedTrees::default();
        let left_zero = integer_expression(&mut program, 0);
        let right_zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[facts::PlaceSegment::Index {
                expression: right_zero,
            }],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn indexed_segment_overlap_is_conservative_for_non_literal_indices() {
        let mut program = typed_trees::TypedTrees::default();
        let left =
            program
                .expression_table
                .insert(checked_trees::expression::ExpressionNode::Name(
                    checked_trees::expression::TableNamePath::default(),
                ));
        let right =
            program
                .expression_table
                .insert(checked_trees::expression::ExpressionNode::Name(
                    checked_trees::expression::TableNamePath::default(),
                ));

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[facts::PlaceSegment::Index { expression: left }],
            &[facts::PlaceSegment::Index { expression: right }],
        ));
    }

    #[test]
    fn fixed_ranges_use_half_open_overlap() {
        let program = typed_trees::TypedTrees::default();
        let range = |start, end| facts::PlaceSegment::FixedRange { start, end };

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(1, 3)],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[range(0, 2)],
            &[range(2, 4)],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[range(1, 1)],
            &[range(0, 2)],
        ));
        assert!(canonical_place_segments_may_overlap(
            &program,
            &[range(1, 3)],
            &[facts::PlaceSegment::FixedIndex { index: 2 }],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[range(1, 3)],
            &[facts::PlaceSegment::FixedIndex { index: 3 }],
        ));
    }
}
