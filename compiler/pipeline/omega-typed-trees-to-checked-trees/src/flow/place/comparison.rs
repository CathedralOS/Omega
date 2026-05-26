pub(crate) fn canonical_place_segments_equal(
    left: omega_facts::PlaceSegment,
    right: omega_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            omega_facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            omega_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            omega_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            omega_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => left_expression == right_expression,
        _ => false,
    }
}

use omega_checked_trees::expression::ExpressionHandle;

#[allow(dead_code)]
pub(crate) fn canonical_place_overlaps_segments(
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
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
    prefix: &[omega_facts::PlaceSegment],
    suffix: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
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
    program: &omega_typed_trees::TypedTrees,
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
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
    program: &omega_typed_trees::TypedTrees,
    prefix: &[omega_facts::PlaceSegment],
    suffix: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
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
    program: &omega_typed_trees::TypedTrees,
    left: omega_facts::PlaceSegment,
    right: omega_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            omega_facts::PlaceSegment::Field {
                symbol: left_symbol,
            },
            omega_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            omega_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            omega_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => index_expressions_may_overlap(program, left_expression, right_expression),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_expression(
        program: &mut omega_typed_trees::TypedTrees,
        value: i64,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(omega_checked_trees::expression::ExpressionNode::Integer(
                value,
            ))
    }

    #[test]
    fn indexed_segment_overlap_uses_literal_values_not_handle_identity() {
        let mut program = omega_typed_trees::TypedTrees::default();
        let left_zero = integer_expression(&mut program, 0);
        let right_zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[omega_facts::PlaceSegment::Index {
                expression: right_zero,
            }],
        ));
        assert!(!canonical_place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::Index {
                expression: left_zero,
            }],
            &[omega_facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn indexed_segment_overlap_is_conservative_for_non_literal_indices() {
        let mut program = omega_typed_trees::TypedTrees::default();
        let left =
            program
                .expression_table
                .insert(omega_checked_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath::default(),
                ));
        let right =
            program
                .expression_table
                .insert(omega_checked_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath::default(),
                ));

        assert!(canonical_place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::Index { expression: left }],
            &[omega_facts::PlaceSegment::Index { expression: right }],
        ));
    }
}

fn index_expressions_may_overlap(
    program: &omega_typed_trees::TypedTrees,
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
            omega_checked_trees::expression::ExpressionNode::Integer(left_value),
            omega_checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => left_value == right_value,
        _ => true,
    }
}
