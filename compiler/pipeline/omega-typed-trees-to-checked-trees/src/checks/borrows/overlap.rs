use omega_checked_trees::expression::ExpressionHandle;

pub(super) fn canonical_place_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &omega_checked_trees::BorrowLoanFact,
    borrow: &omega_checked_trees::BorrowFacts,
) -> bool {
    match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                return place_segments_may_overlap(
                    program,
                    &place.segments,
                    borrow.loan_segments(loan),
                );
            }

            match place.segments.split_first() {
                Some((
                    omega_facts::PlaceSegment::Field {
                        symbol: field_symbol,
                    },
                    remaining,
                )) if *field_symbol == loan.root_symbol => {
                    place_segments_may_overlap(program, remaining, borrow.loan_segments(loan))
                }
                _ => false,
            }
        }
        omega_facts::PlaceRoot::Unknown
        | omega_facts::PlaceRoot::Expression(_)
        | omega_facts::PlaceRoot::TypeReference(_) => false,
    }
}

pub(super) fn borrow_accesses_overlap(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    left: &omega_checked_trees::BorrowArgumentAccessFact,
    right: &omega_checked_trees::BorrowArgumentAccessFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(left),
            facts.borrow.access_segments(right),
        )
}

pub(super) fn borrow_access_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    access: &omega_checked_trees::BorrowArgumentAccessFact,
    loan: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    access.root_symbol == loan.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(access),
            facts.borrow.loan_segments(loan),
        )
}

pub(super) fn borrow_loan_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    left: &omega_checked_trees::BorrowLoanFact,
    right: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.loan_segments(left),
            facts.borrow.loan_segments(right),
        )
}

fn place_segments_may_overlap(
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
        (
            omega_checked_trees::expression::ExpressionNode::Range(left_range),
            omega_checked_trees::expression::ExpressionNode::Integer(right_value),
        ) => range_may_contain_integer(program, left_range, *right_value),
        (
            omega_checked_trees::expression::ExpressionNode::Integer(left_value),
            omega_checked_trees::expression::ExpressionNode::Range(right_range),
        ) => range_may_contain_integer(program, right_range, *left_value),
        (
            omega_checked_trees::expression::ExpressionNode::Range(left_range),
            omega_checked_trees::expression::ExpressionNode::Range(right_range),
        ) => ranges_may_overlap(program, left_range, right_range),
        _ => true,
    }
}

fn range_may_contain_integer(
    program: &omega_typed_trees::TypedTrees,
    range: &omega_checked_trees::expression::TableRangeExpression,
    value: i64,
) -> bool {
    let (start, end) = range_integer_bounds(program, range);
    if start.is_some_and(|start| value < start) {
        return false;
    }
    if end.is_some_and(|end| value >= end) {
        return false;
    }
    true
}

fn ranges_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &omega_checked_trees::expression::TableRangeExpression,
    right: &omega_checked_trees::expression::TableRangeExpression,
) -> bool {
    let (left_start, left_end) = range_integer_bounds(program, left);
    let (right_start, right_end) = range_integer_bounds(program, right);

    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && left_end <= right_start
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && right_end <= left_start
    {
        return false;
    }
    true
}

fn range_integer_bounds(
    program: &omega_typed_trees::TypedTrees,
    range: &omega_checked_trees::expression::TableRangeExpression,
) -> (Option<i64>, Option<i64>) {
    (
        integer_expression_value(program, range.start),
        integer_expression_value(program, range.end),
    )
}

fn integer_expression_value(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        omega_checked_trees::expression::ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}
