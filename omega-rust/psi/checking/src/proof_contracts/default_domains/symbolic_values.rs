//! Conservative symbolic evaluation for default-domain invariant checks.

use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SymbolicValue {
    Atom(symbols::SymbolHandle),
    Integer(i128),
    Add(Vec<SymbolicValue>),
    Multiply(Vec<SymbolicValue>),
    Subtract(Box<SymbolicValue>, Box<SymbolicValue>),
}

pub(super) fn integer_literal_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<i128> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Borrow(inner) => integer_literal_value(program, inner.target),
        _ => None,
    }
}

/// Fold a where fact over the tracked valuation: tracked fields read their
/// value (a non-literal write poisons), untracked fields read the ZII zero
/// (machine-owned data is born zeroed).
pub(super) fn fold_with_valuation(
    program: &TypedTrees,
    valuation: &[(&str, Option<i128>)],
    symbols: &[(String, SymbolicValue)],
    measures: &[(String, Option<i128>, Option<i128>)],
    born_zero: bool,
    expression: ExpressionHandle,
) -> Option<i128> {
    use typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let last = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match valuation.iter().find(|(name, _)| *name == last) {
                Some((_, value)) => *value,
                // SOUNDNESS (slice 4): the born zero is real only in the
                // never-re-entered boot state; elsewhere an untracked field
                // may hold any prior value -- poison the fold.
                None if born_zero => Some(0),
                None => None,
            }
        }
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
            let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let field = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match measures.iter().find(|(name, _, _)| name == field) {
                Some((_, length, capacity)) => {
                    match CollectionMeasure::from_authored_spelling(member.member.as_str()) {
                        Some(CollectionMeasure::Length) => *length,
                        Some(CollectionMeasure::Capacity) => *capacity,
                        None => None,
                    }
                }
                None if born_zero => Some(0),
                None => None,
            }
        }
        ExpressionNode::Binary(binary) => {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) && let (Some(left), Some(right)) = (
                symbolic_operand(program, symbols, binary.left),
                symbolic_operand(program, symbols, binary.right),
            ) && left == right
            {
                return Some(i128::from(matches!(binary.operator, BinaryOperator::Equal)));
            }
            let left = fold_with_valuation(
                program,
                valuation,
                symbols,
                measures,
                born_zero,
                binary.left,
            )?;
            let right = fold_with_valuation(
                program,
                valuation,
                symbols,
                measures,
                born_zero,
                binary.right,
            )?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn expression_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    path.symbol.is_valid().then_some(path.symbol)
}

pub(super) fn expression_symbolic_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolicValue> {
    use typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            Some(SymbolicValue::Atom(path.symbol))
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i128>()
            .ok()
            .map(SymbolicValue::Integer),
        ExpressionNode::Binary(binary) => {
            let left = expression_symbolic_value(program, binary.left)?;
            let right = expression_symbolic_value(program, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some(commutative_symbolic_value(true, left, right)),
                BinaryOperator::Multiply => Some(commutative_symbolic_value(false, left, right)),
                BinaryOperator::Subtract => {
                    Some(SymbolicValue::Subtract(Box::new(left), Box::new(right)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn commutative_symbolic_value(
    add: bool,
    left: SymbolicValue,
    right: SymbolicValue,
) -> SymbolicValue {
    // Canonicalize commutation only. Do not flatten/reassociate: signed
    // saturating addition is commutative but not associative, and symbolic
    // provenance must not silently strengthen the selected arithmetic theory.
    let mut operands = vec![left, right];
    operands.sort_by_key(symbolic_sort_key);
    if add {
        SymbolicValue::Add(operands)
    } else {
        SymbolicValue::Multiply(operands)
    }
}

fn symbolic_sort_key(value: &SymbolicValue) -> String {
    match value {
        SymbolicValue::Atom(symbol) => {
            format!("a:{}:{}", symbol.arena_index(), symbol.generation())
        }
        SymbolicValue::Integer(value) => format!("i:{value}"),
        SymbolicValue::Add(values) => format!(
            "+({})",
            values
                .iter()
                .map(symbolic_sort_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        SymbolicValue::Multiply(values) => format!(
            "*({})",
            values
                .iter()
                .map(symbolic_sort_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        SymbolicValue::Subtract(left, right) => {
            format!(
                "-({},{})",
                symbolic_sort_key(left),
                symbolic_sort_key(right)
            )
        }
    }
}

fn symbolic_operand(
    program: &TypedTrees,
    symbols: &[(String, SymbolicValue)],
    expression: ExpressionHandle,
) -> Option<SymbolicValue> {
    use typed_trees::expression::BinaryOperator;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let field = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            symbols
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| value.clone())
                .or_else(|| {
                    path.symbol
                        .is_valid()
                        .then_some(SymbolicValue::Atom(path.symbol))
                })
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i128>()
            .ok()
            .map(SymbolicValue::Integer),
        ExpressionNode::Binary(binary) => {
            let left = symbolic_operand(program, symbols, binary.left)?;
            let right = symbolic_operand(program, symbols, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some(commutative_symbolic_value(true, left, right)),
                BinaryOperator::Multiply => Some(commutative_symbolic_value(false, left, right)),
                BinaryOperator::Subtract => {
                    Some(SymbolicValue::Subtract(Box::new(left), Box::new(right)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn expression_sequence_measures(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(Option<i128>, Option<i128>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => {
            let measure = i128::try_from(literal.len()).ok();
            Some((measure, measure))
        }
        _ => None,
    }
}

/// Whether ANY reachable nested expression is a call. A call is a hard
/// consumption point for open invariant windows, so the traversal must cover
/// every expression shape that can carry one -- casts, unary operands,
/// indices, literals' elements and fields, match subjects and arms, ranges,
/// and atomic value/result -- not merely the binary/member/breadth spine.
pub(super) fn expression_contains_call(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(program, binary.left)
                || expression_contains_call(program, binary.right)
        }
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_contains_call(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(program, indexed.collection)
                || expression_contains_call(program, indexed.index)
        }
        ExpressionNode::Cast(cast) => expression_contains_call(program, cast.value),
        ExpressionNode::Unary(unary) => expression_contains_call(program, unary.operand),
        ExpressionNode::Atomic(atomic) => {
            expression_contains_call(program, atomic.value)
                || expression_contains_call(program, atomic.result)
        }
        ExpressionNode::Range(range) => {
            expression_contains_call(program, range.start)
                || expression_contains_call(program, range.end)
        }
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_contains_call(program, *element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_call(program, field.value)),
        ExpressionNode::Match(matched) => {
            expression_contains_call(program, matched.subject)
                || program
                    .expression_table
                    .match_arms(matched.arms)
                    .iter()
                    .any(|arm| {
                        expression_contains_call(program, arm.value)
                            || match arm.pattern {
                                typed_trees::expression::MatchPattern::Value(pattern) => {
                                    expression_contains_call(program, pattern)
                                }
                                typed_trees::expression::MatchPattern::Wildcard => false,
                            }
                    })
        }
        _ => false,
    }
}
