pub(crate) fn canonical_place_segments_equal(
    left: psi_facts::PlaceSegment,
    right: psi_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            psi_facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            psi_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            psi_facts::PlaceSegment::Case {
                variant: left_variant,
            },
            psi_facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            psi_facts::PlaceSegment::FixedIndex { index: left_index },
            psi_facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            psi_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            psi_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => left_expression == right_expression,
        _ => false,
    }
}

use psi_checked_trees::expression::ExpressionHandle;

#[allow(dead_code)]
pub(crate) fn canonical_place_overlaps_segments(
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
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
    prefix: &[psi_facts::PlaceSegment],
    suffix: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
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
    program: &psi_typed_trees::TypedTrees,
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
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
    program: &psi_typed_trees::TypedTrees,
    prefix: &[psi_facts::PlaceSegment],
    suffix: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
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
    program: &psi_typed_trees::TypedTrees,
    left: psi_facts::PlaceSegment,
    right: psi_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            psi_facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            psi_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            psi_facts::PlaceSegment::Case {
                variant: left_variant,
            },
            psi_facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            psi_facts::PlaceSegment::FixedIndex { index: left_index },
            psi_facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            psi_facts::PlaceSegment::FixedIndex { index },
            psi_facts::PlaceSegment::Index { expression },
        )
        | (
            psi_facts::PlaceSegment::Index { expression },
            psi_facts::PlaceSegment::FixedIndex { index },
        ) => expression_static_index(program, expression).is_none_or(|value| value == index),
        (
            psi_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            psi_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap(program, left_expression, right_expression),
        _ => false,
    }
}

fn expression_static_index(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<usize> {
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_expression(
        program: &mut psi_typed_trees::TypedTrees,
        value: i64,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(psi_checked_trees::expression::ExpressionNode::Integer(
                psi_numerics::literals::IntegerLiteral::from_value(value),
            ))
    }

    #[test]
    fn indexed_segment_overlap_uses_literal_values_not_handle_identity() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let left_zero = integer_expression(&mut program, 0);
        let right_zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[psi_facts::PlaceSegment::Index {
                expression: right_zero,
            }],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[psi_facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn indexed_segment_overlap_is_conservative_for_non_literal_indices() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let left =
            program
                .expression_table
                .insert(psi_checked_trees::expression::ExpressionNode::Name(
                    psi_checked_trees::expression::TableNamePath::default(),
                ));
        let right =
            program
                .expression_table
                .insert(psi_checked_trees::expression::ExpressionNode::Name(
                    psi_checked_trees::expression::TableNamePath::default(),
                ));

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::Index { expression: left }],
            &[psi_facts::PlaceSegment::Index { expression: right }],
        ));
    }
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
    program: &psi_typed_trees::TypedTrees,
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
            psi_checked_trees::expression::ExpressionNode::Integer(left_value),
            psi_checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => left_value == right_value,
        _ => true,
    }
}
