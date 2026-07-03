use omega_checked_trees::expression::{BinaryExpression, BinaryOperator, Expression};
use omega_core::symbols::SymbolHandle;

pub(super) fn fold_binary_expression(
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
) -> Expression {
    use BinaryOperator as Op;

    match operator {
        Op::And => boolean_and(left, right),
        Op::Or => boolean_or(left, right),
        Op::Equal | Op::NotEqual => {
            if let Expression::Boolean(flag) = right {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { left } else { boolean_not(left) };
            }
            if let Expression::Boolean(flag) = left {
                let positive = matches!(operator, Op::Equal) == flag;
                return if positive { right } else { boolean_not(right) };
            }
            // NOTE: the REFLEXIVE fold (structurally-equal non-literal
            // operands) is NOT here -- it lives in
            // `simplify_binary_expression`, TYPE-GATED, because
            // `x == x -> true` / `x != x -> false` is an INVALID identity for
            // floats (IEEE: NaN != NaN is TRUE; the canonical isNaN idiom
            // `f != f` was silently folded to `false` before the gate).
            match operator {
                Op::Equal => match (&left, &right) {
                    (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a == b),
                    (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a == b),
                    (Expression::String(a), Expression::String(b)) => Expression::Boolean(a == b),
                    _ => Expression::Binary(Box::new(BinaryExpression {
                        left,
                        operator,
                        right,
                    })),
                },
                Op::NotEqual => match (&left, &right) {
                    (Expression::Boolean(a), Expression::Boolean(b)) => Expression::Boolean(a != b),
                    (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(a != b),
                    (Expression::String(a), Expression::String(b)) => Expression::Boolean(a != b),
                    _ => Expression::Binary(Box::new(BinaryExpression {
                        left,
                        operator,
                        right,
                    })),
                },
                _ => unreachable!(),
            }
        }
        Op::Greater => fold_integer_compare(left, right, |a, b| a > b, operator),
        Op::GreaterOrEqual => fold_integer_compare(left, right, |a, b| a >= b, operator),
        Op::Less => fold_integer_compare(left, right, |a, b| a < b, operator),
        Op::LessOrEqual => fold_integer_compare(left, right, |a, b| a <= b, operator),
        Op::Add => fold_integer_math(left, right, |a, b| a + b, operator),
        Op::Subtract => fold_integer_math(left, right, |a, b| a - b, operator),
        Op::Multiply => fold_integer_math(left, right, |a, b| a * b, operator),
        Op::Divide => match (&left, &right) {
            (Expression::Integer(_), Expression::Integer(0)) => {
                Expression::Binary(Box::new(BinaryExpression {
                    left,
                    operator,
                    right,
                }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a / b),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        Op::Modulo => match (&left, &right) {
            (Expression::Integer(_), Expression::Integer(0)) => {
                Expression::Binary(Box::new(BinaryExpression {
                    left,
                    operator,
                    right,
                }))
            }
            (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(a % b),
            _ => Expression::Binary(Box::new(BinaryExpression {
                left,
                operator,
                right,
            })),
        },
        Op::ShiftLeft => fold_integer_math(left, right, |a, b| a << b, operator),
        Op::ShiftRight => fold_integer_math(left, right, |a, b| a >> b, operator),
        Op::BitwiseAnd => fold_integer_math(left, right, |a, b| a & b, operator),
        Op::BitwiseOr => fold_integer_math(left, right, |a, b| a | b, operator),
        Op::BitwiseXor => fold_integer_math(left, right, |a, b| a ^ b, operator),
    }
}

fn fold_integer_math(
    left: Expression,
    right: Expression,
    operation: impl FnOnce(i64, i64) -> i64,
    operator: BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Integer(operation(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

fn fold_integer_compare(
    left: Expression,
    right: Expression,
    comparison: impl FnOnce(i64, i64) -> bool,
    operator: BinaryOperator,
) -> Expression {
    match (&left, &right) {
        (Expression::Integer(a), Expression::Integer(b)) => Expression::Boolean(comparison(*a, *b)),
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator,
            right,
        })),
    }
}

pub(super) fn boolean_and(left: Expression, right: Expression) -> Expression {
    if let Expression::Binary(binary) = &left
        && binary.operator == BinaryOperator::Or
    {
        return boolean_or(
            boolean_and(binary.left.clone(), right.clone()),
            boolean_and(binary.right.clone(), right),
        );
    }

    if let Expression::Binary(binary) = &right
        && binary.operator == BinaryOperator::Or
    {
        return boolean_or(
            boolean_and(left.clone(), binary.left.clone()),
            boolean_and(left, binary.right.clone()),
        );
    }

    if let Some(simplified) = simplify_comparison_conjunction(&left, &right) {
        return simplified;
    }

    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
            Expression::Boolean(false)
        }
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        })),
    }
}

pub(super) fn boolean_or(left: Expression, right: Expression) -> Expression {
    if let Some(simplified) = simplify_comparison_disjunction(&left, &right) {
        return simplified;
    }

    if let Some(factored) = factor_common_conjuncts(&left, &right) {
        return factored;
    }

    match (&left, &right) {
        (Expression::Boolean(true), _) | (_, Expression::Boolean(true)) => {
            Expression::Boolean(true)
        }
        (Expression::Boolean(false), _) => right,
        (_, Expression::Boolean(false)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::Or,
            right,
        })),
    }
}

fn factor_common_conjuncts(left: &Expression, right: &Expression) -> Option<Expression> {
    let mut left_conjuncts = Vec::new();
    let mut right_conjuncts = Vec::new();
    collect_conjuncts(left, &mut left_conjuncts);
    collect_conjuncts(right, &mut right_conjuncts);

    let mut common = Vec::new();
    let mut remaining_right = right_conjuncts;

    for left_conjunct in left_conjuncts {
        if let Some(index) = remaining_right
            .iter()
            .position(|candidate| expressions_equivalent(left_conjunct, candidate))
        {
            common.push(left_conjunct.clone());
            remaining_right.remove(index);
        }
    }

    if common.is_empty() {
        return None;
    }

    let mut remaining_left = Vec::new();
    collect_unique_non_common_conjuncts(left, &common, &mut remaining_left);

    let left_rest = combine_conjuncts(remaining_left);
    let right_rest = combine_conjuncts(remaining_right.into_iter().cloned().collect::<Vec<_>>());
    let mut factored = boolean_or(left_rest, right_rest);
    for conjunct in common.into_iter().rev() {
        // NOT `boolean_and`: that distributes the conjunct back over the
        // disjunction it was just factored out of, and `boolean_or` would
        // then factor it again -- non-terminating mutual recursion (first
        // reachable through mixed-shape equality, whose synthesized
        // `common-field compares && (case arms...)` repeats the common
        // compares across every disjunction arm).
        factored = conjoin_without_distribution(conjunct, factored);
    }
    Some(factored)
}

/// `boolean_and` minus the distribute-over-`Or` rewrite: the constant,
/// duplicate, and comparison-conjunction rules only, never recursing into
/// `boolean_or`. Used where an And node must wrap a disjunction AS-IS
/// (re-attaching factored common conjuncts).
fn conjoin_without_distribution(left: Expression, right: Expression) -> Expression {
    if let Some(simplified) = simplify_comparison_conjunction(&left, &right) {
        return simplified;
    }

    match (&left, &right) {
        (Expression::Boolean(false), _) | (_, Expression::Boolean(false)) => {
            Expression::Boolean(false)
        }
        (Expression::Boolean(true), _) => right,
        (_, Expression::Boolean(true)) => left,
        _ if left == right => left,
        _ => Expression::Binary(Box::new(BinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        })),
    }
}

fn simplify_comparison_conjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    let mut lower_bound = None;
    let mut upper_bound = None;

    for comparison in [left_compare, right_compare] {
        match comparison.operator {
            BinaryOperator::Greater => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, false);
            }
            BinaryOperator::GreaterOrEqual => {
                lower_bound = tighten_lower_bound(lower_bound, comparison.value, true);
            }
            BinaryOperator::Less => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, false);
            }
            BinaryOperator::LessOrEqual => {
                upper_bound = tighten_upper_bound(upper_bound, comparison.value, true);
            }
            _ => return None,
        }
    }

    if let (Some((lower, lower_inclusive)), Some((upper, upper_inclusive))) =
        (lower_bound, upper_bound)
    {
        let impossible =
            lower > upper || (lower == upper && (!lower_inclusive || !upper_inclusive));
        if impossible {
            return Some(Expression::Boolean(false));
        }
    }

    if lower_bound.is_some() && upper_bound.is_none() {
        let (value, inclusive) = lower_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                BinaryOperator::GreaterOrEqual
            } else {
                BinaryOperator::Greater
            },
            right: Expression::Integer(value),
        })));
    }

    if upper_bound.is_some() && lower_bound.is_none() {
        let (value, inclusive) = upper_bound?;
        return Some(Expression::Binary(Box::new(BinaryExpression {
            left: left_compare.subject.clone(),
            operator: if inclusive {
                BinaryOperator::LessOrEqual
            } else {
                BinaryOperator::Less
            },
            right: Expression::Integer(value),
        })));
    }

    None
}

fn simplify_comparison_disjunction(left: &Expression, right: &Expression) -> Option<Expression> {
    let left_compare = parse_integer_comparison(left)?;
    let right_compare = parse_integer_comparison(right)?;

    if !expressions_equivalent(left_compare.subject, right_compare.subject) {
        return None;
    }

    if left_compare.operator == right_compare.operator && left_compare.value == right_compare.value
    {
        return Some(left.clone());
    }

    use BinaryOperator as Op;
    match (left_compare.operator, right_compare.operator) {
        (Op::Greater, Op::Greater)
        | (Op::Greater, Op::GreaterOrEqual)
        | (Op::GreaterOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::GreaterOrEqual) => {
            let (value, inclusive) = loosen_lower_bound_for_disjunction(
                (
                    left_compare.value,
                    left_compare.operator == Op::GreaterOrEqual,
                ),
                (
                    right_compare.value,
                    right_compare.operator == Op::GreaterOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive {
                    Op::GreaterOrEqual
                } else {
                    Op::Greater
                },
                right: Expression::Integer(value),
            })))
        }
        (Op::Less, Op::Less)
        | (Op::Less, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Less)
        | (Op::LessOrEqual, Op::LessOrEqual) => {
            let (value, inclusive) = loosen_upper_bound_for_disjunction(
                (left_compare.value, left_compare.operator == Op::LessOrEqual),
                (
                    right_compare.value,
                    right_compare.operator == Op::LessOrEqual,
                ),
            );
            Some(Expression::Binary(Box::new(BinaryExpression {
                left: left_compare.subject.clone(),
                operator: if inclusive { Op::LessOrEqual } else { Op::Less },
                right: Expression::Integer(value),
            })))
        }
        (Op::Greater, Op::LessOrEqual)
        | (Op::LessOrEqual, Op::Greater)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Less, Op::GreaterOrEqual) => {
            if comparisons_cover_all_integers(left_compare, right_compare) {
                Some(Expression::Boolean(true))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct IntegerComparison<'expression> {
    subject: &'expression Expression,
    operator: BinaryOperator,
    value: i64,
}

fn parse_integer_comparison(expression: &Expression) -> Option<IntegerComparison<'_>> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    let operator = match binary.operator {
        BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual => binary.operator,
        _ => return None,
    };

    if let Expression::Integer(value) = &binary.right {
        return Some(IntegerComparison {
            subject: &binary.left,
            operator,
            value: *value,
        });
    }

    if let Expression::Integer(value) = &binary.left {
        let flipped_operator = match binary.operator {
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            _ => unreachable!(),
        };

        return Some(IntegerComparison {
            subject: &binary.right,
            operator: flipped_operator,
            value: *value,
        });
    }

    None
}

fn tighten_lower_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value > current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value < current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn tighten_upper_bound(
    current: Option<(i64, bool)>,
    candidate_value: i64,
    candidate_inclusive: bool,
) -> Option<(i64, bool)> {
    match current {
        None => Some((candidate_value, candidate_inclusive)),
        Some((current_value, current_inclusive)) => {
            if candidate_value < current_value {
                Some((candidate_value, candidate_inclusive))
            } else if candidate_value > current_value {
                Some((current_value, current_inclusive))
            } else {
                Some((current_value, current_inclusive && candidate_inclusive))
            }
        }
    }
}

fn loosen_lower_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => left,
        std::cmp::Ordering::Greater => right,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn loosen_upper_bound_for_disjunction(left: (i64, bool), right: (i64, bool)) -> (i64, bool) {
    match left.0.cmp(&right.0) {
        std::cmp::Ordering::Less => right,
        std::cmp::Ordering::Greater => left,
        std::cmp::Ordering::Equal => (left.0, left.1 || right.1),
    }
}

fn comparisons_cover_all_integers(
    left: IntegerComparison<'_>,
    right: IntegerComparison<'_>,
) -> bool {
    use BinaryOperator as Op;

    let (lower, upper) = match (left.operator, right.operator) {
        (Op::Greater, Op::LessOrEqual)
        | (Op::GreaterOrEqual, Op::Less)
        | (Op::Greater, Op::Less)
        | (Op::GreaterOrEqual, Op::LessOrEqual) => (left, right),
        (Op::LessOrEqual, Op::Greater)
        | (Op::Less, Op::GreaterOrEqual)
        | (Op::Less, Op::Greater)
        | (Op::LessOrEqual, Op::GreaterOrEqual) => (right, left),
        _ => return false,
    };

    let lower_min = match lower.operator {
        Op::Greater => lower.value + 1,
        Op::GreaterOrEqual => lower.value,
        _ => return false,
    };
    let upper_max = match upper.operator {
        Op::Less => upper.value - 1,
        Op::LessOrEqual => upper.value,
        _ => return false,
    };
    lower_min <= upper_max + 1
}

fn collect_conjuncts<'a>(expression: &'a Expression, output: &mut Vec<&'a Expression>) {
    if let Expression::Binary(binary) = expression
        && binary.operator == BinaryOperator::And
    {
        collect_conjuncts(&binary.left, output);
        collect_conjuncts(&binary.right, output);
        return;
    }
    output.push(expression);
}

fn collect_unique_non_common_conjuncts(
    expression: &Expression,
    common: &[Expression],
    output: &mut Vec<Expression>,
) {
    let mut conjuncts = Vec::new();
    collect_conjuncts(expression, &mut conjuncts);
    let mut remaining_common = common.to_vec();
    for conjunct in conjuncts {
        if let Some(index) = remaining_common
            .iter()
            .position(|candidate| expressions_equivalent(conjunct, candidate))
        {
            remaining_common.remove(index);
        } else {
            output.push(conjunct.clone());
        }
    }
}

fn combine_conjuncts(conjuncts: Vec<Expression>) -> Expression {
    conjuncts
        .into_iter()
        .reduce(boolean_and)
        .unwrap_or(Expression::Boolean(true))
}

pub(super) fn boolean_not(expression: Expression) -> Expression {
    use BinaryOperator as Op;

    match expression {
        Expression::Boolean(value) => Expression::Boolean(!value),
        Expression::Binary(binary) => {
            let inverted = match binary.operator {
                Op::Equal => Some(Op::NotEqual),
                Op::NotEqual => Some(Op::Equal),
                Op::Greater => Some(Op::LessOrEqual),
                Op::GreaterOrEqual => Some(Op::Less),
                Op::Less => Some(Op::GreaterOrEqual),
                Op::LessOrEqual => Some(Op::Greater),
                Op::And => {
                    return boolean_or(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Or => {
                    return boolean_and(boolean_not(binary.left), boolean_not(binary.right));
                }
                Op::Add
                | Op::BitwiseAnd
                | Op::BitwiseOr
                | Op::BitwiseXor
                | Op::Divide
                | Op::Modulo
                | Op::Multiply
                | Op::ShiftLeft
                | Op::ShiftRight
                | Op::Subtract => None,
            };

            if let Some(operator) = inverted {
                Expression::Binary(Box::new(BinaryExpression {
                    left: binary.left,
                    operator,
                    right: binary.right,
                }))
            } else {
                Expression::Binary(Box::new(BinaryExpression {
                    left: Expression::Binary(binary),
                    operator: Op::Equal,
                    right: Expression::Boolean(false),
                }))
            }
        }
        other => Expression::Binary(Box::new(BinaryExpression {
            left: other,
            operator: BinaryOperator::Equal,
            right: Expression::Boolean(false),
        })),
    }
}

pub(super) fn expressions_equivalent(left: &Expression, right: &Expression) -> bool {
    if let Some(are_equivalent) = expression_paths_equivalent(left, right) {
        return are_equivalent;
    }

    match (left, right) {
        (Expression::ArrayLiteral(left), Expression::ArrayLiteral(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Binary(left), Expression::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent(&left.left, &right.left)
                && expressions_equivalent(&left.right, &right.right)
        }
        (Expression::Boolean(left), Expression::Boolean(right)) => left == right,
        (Expression::Call(left), Expression::Call(right)) => {
            left.target == right.target
                && left.arguments.len() == right.arguments.len()
                && left
                    .receiver
                    .as_deref()
                    .zip(right.receiver.as_deref())
                    .map(|(left, right)| expressions_equivalent(left, right))
                    .unwrap_or(left.receiver.is_none() && right.receiver.is_none())
                && left
                    .arguments
                    .iter()
                    .zip(right.arguments.iter())
                    .all(|(left, right)| expressions_equivalent(left, right))
        }
        (Expression::Cast(left), Expression::Cast(right)) => {
            left.target_type.symbol().is_valid()
                && right.target_type.symbol().is_valid()
                && left.target_type.symbol() == right.target_type.symbol()
                && expressions_equivalent(&left.value, &right.value)
        }
        (Expression::Float(left), Expression::Float(right)) => left == right,
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expressions_equivalent(&left.collection, &right.collection)
                && expressions_equivalent(&left.index, &right.index)
        }
        (Expression::Integer(left), Expression::Integer(right)) => left == right,
        (Expression::Member(left), Expression::Member(right)) => {
            ((left.member_symbol.is_valid()
                && right.member_symbol.is_valid()
                && left.member_symbol == right.member_symbol)
                || (!left.member_symbol.is_valid()
                    && !right.member_symbol.is_valid()
                    && left.member == right.member))
                && expressions_equivalent(&left.receiver, &right.receiver)
        }
        (Expression::Mutable(left), Expression::Mutable(right)) => {
            expressions_equivalent(left, right)
        }
        (Expression::Name(left), Expression::Name(right)) => {
            left.symbol().is_valid() && right.symbol().is_valid() && left.symbol() == right.symbol()
        }
        (Expression::StructLiteral(left), Expression::StructLiteral(right)) => {
            left.type_name == right.type_name
                && left.fields.len() == right.fields.len()
                && left
                    .fields
                    .iter()
                    .zip(right.fields.iter())
                    .all(|(left, right)| {
                        left.name == right.name && expressions_equivalent(&left.value, &right.value)
                    })
        }
        (Expression::String(left), Expression::String(right)) => left == right,
        _ => false,
    }
}

fn expression_paths_equivalent(left: &Expression, right: &Expression) -> Option<bool> {
    let left_count = expression_path_segment_count(left)?;
    let right_count = expression_path_segment_count(right)?;

    if left_count != right_count {
        return Some(false);
    }

    Some((0..left_count).all(|index| {
        let left_symbol = expression_path_segment_symbol(left, index);
        let right_symbol = expression_path_segment_symbol(right, index);
        left_symbol.is_valid() && right_symbol.is_valid() && left_symbol == right_symbol
    }))
}

fn expression_path_segment_count(expression: &Expression) -> Option<usize> {
    match expression {
        Expression::Name(path) => Some(path.len()),
        Expression::Member(member) => Some(expression_path_segment_count(&member.receiver)? + 1),
        _ => None,
    }
}

fn expression_path_segment_symbol(expression: &Expression, index: usize) -> SymbolHandle {
    match expression {
        Expression::Name(path) => path.member_symbol(index),
        Expression::Member(member) => {
            let Some(receiver_count) = expression_path_segment_count(&member.receiver) else {
                return SymbolHandle::invalid();
            };
            if index == receiver_count {
                member.member_symbol
            } else {
                expression_path_segment_symbol(&member.receiver, index)
            }
        }
        _ => SymbolHandle::invalid(),
    }
}
