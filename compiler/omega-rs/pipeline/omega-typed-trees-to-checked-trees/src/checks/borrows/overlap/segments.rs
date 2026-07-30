use super::indexes::{index_expression_may_contain_fixed, index_expressions_may_overlap};

pub(super) fn place_segments_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            place_segment_pair_may_overlap(program, *left_segment, *right_segment)
        })
}

fn place_segment_pair_may_overlap(
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
            omega_facts::PlaceSegment::Case {
                variant: left_variant,
            },
            omega_facts::PlaceSegment::Case {
                variant: right_variant,
            },
        ) => left_variant == right_variant,
        (
            omega_facts::PlaceSegment::FixedIndex { index: left_index },
            omega_facts::PlaceSegment::FixedIndex { index: right_index },
        ) => left_index == right_index,
        (
            omega_facts::PlaceSegment::FixedIndex { index },
            omega_facts::PlaceSegment::Index { expression },
        )
        | (
            omega_facts::PlaceSegment::Index { expression },
            omega_facts::PlaceSegment::FixedIndex { index },
        ) => index_expression_may_contain_fixed(program, expression, index),
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
    use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode};

    fn integer_expression(
        program: &mut omega_typed_trees::TypedTrees,
        value: i64,
    ) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            omega_core::literals::IntegerLiteral::from_value(value),
        ))
    }

    #[test]
    fn fixed_indices_overlap_only_the_same_element() {
        let program = omega_typed_trees::TypedTrees::default();

        assert!(place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[omega_facts::PlaceSegment::FixedIndex { index: 1 }],
        ));
    }

    #[test]
    fn fixed_and_legacy_literal_indices_compare_by_value() {
        let mut program = omega_typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[omega_facts::PlaceSegment::Index { expression: zero }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[omega_facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn fixed_index_before_tail_range_is_disjoint() {
        let mut program = omega_typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let tail = program.expression_table.insert(ExpressionNode::Range(
            omega_typed_trees::expression::TableRangeExpression {
                start: one,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            },
        ));

        assert!(!place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[omega_facts::PlaceSegment::Index { expression: tail }],
        ));
        assert!(place_segments_may_overlap(
            &program,
            &[omega_facts::PlaceSegment::FixedIndex { index: 1 }],
            &[omega_facts::PlaceSegment::Index { expression: tail }],
        ));
    }
}
