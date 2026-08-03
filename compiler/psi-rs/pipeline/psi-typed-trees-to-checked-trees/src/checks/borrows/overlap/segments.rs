use super::indexes::{index_expression_may_contain_fixed, index_expressions_may_overlap};

pub(super) fn place_segments_may_overlap(
    program: &psi_typed_trees::TypedTrees,
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
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
        ) => index_expression_may_contain_fixed(program, expression, index),
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode};

    fn integer_expression(
        program: &mut psi_typed_trees::TypedTrees,
        value: i64,
    ) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    #[test]
    fn fixed_indices_overlap_only_the_same_element() {
        let program = psi_typed_trees::TypedTrees::default();

        assert!(place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[psi_facts::PlaceSegment::FixedIndex { index: 1 }],
        ));
    }

    #[test]
    fn fixed_and_legacy_literal_indices_compare_by_value() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);

        assert!(place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[psi_facts::PlaceSegment::Index { expression: zero }],
        ));
        assert!(!place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[psi_facts::PlaceSegment::Index { expression: one }],
        ));
    }

    #[test]
    fn fixed_index_before_tail_range_is_disjoint() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let one = integer_expression(&mut program, 1);
        let tail = program.expression_table.insert(ExpressionNode::Range(
            psi_typed_trees::expression::TableRangeExpression {
                start: one,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            },
        ));

        assert!(!place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 0 }],
            &[psi_facts::PlaceSegment::Index { expression: tail }],
        ));
        assert!(place_segments_may_overlap(
            &program,
            &[psi_facts::PlaceSegment::FixedIndex { index: 1 }],
            &[psi_facts::PlaceSegment::Index { expression: tail }],
        ));
    }
}
