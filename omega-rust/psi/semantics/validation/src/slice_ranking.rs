//! The typed decrease rule shared by runtime and proof slice recursion.

use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceNode;

/// A taken nonempty-slice guard makes its exact parameter's `start..` tail
/// strictly shorter whenever `start` normalizes to an immutable integer bound
/// of at least 1 and the guard proves `parameter.len >= start`.
///
/// Both bounds route through the shared immutable-integer-bound normalization,
/// so a literal (`items[2..]` under `items.len >= 2`) and an immutable local
/// copy (`items[step..]` under `items.len >= step` with `let step = 2`) admit;
/// mutable, computed, or ambiguous bounds stay unknown rather than guessing a
/// value from spelling. A `0..` tail is the whole slice and never decreases,
/// and a guard below `start` cannot establish that the tail is a valid window
/// or a strict decrease. The caller owns guard dominance and binding
/// stability; this predicate does not establish either from matching
/// expression text.
pub fn slice_tail_strictly_decreases(
    program: &TypedTrees,
    guard: ExpressionHandle,
    argument: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    if !parameter.symbol.is_valid() || !parameter_is_slice(program, parameter) {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if !names_parameter(program, indexed.collection, parameter)
        || range.end.is_valid()
        || range.end_inclusive
    {
        return false;
    }
    let Some(start) = crate::normalize_immutable_integer_bound_to_usize(program, range.start)
    else {
        return false;
    };
    // `parameter[start..]` has length `len - start`: strictly shorter than
    // `len` only when `start >= 1`.
    if start < 1 {
        return false;
    }
    let guard = match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if binary.operator == BinaryOperator::Equal
                && matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
        {
            binary.left
        }
        _ => guard,
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    let ExpressionNode::Member(length) = program.expression_table.expression(binary.left) else {
        return false;
    };
    // `len > m` proves `len >= m + 1`, so it discharges `len >= start` when
    // `m >= start - 1`; `len >= m` needs `m >= start` directly.
    length.member.as_str() == "len"
        && names_parameter(program, length.receiver, parameter)
        && match binary.operator {
            BinaryOperator::Greater => normalized_bound_at_least(program, binary.right, start - 1),
            BinaryOperator::GreaterOrEqual => {
                normalized_bound_at_least(program, binary.right, start)
            }
            _ => false,
        }
}

/// A guard bound that normalizes to an immutable integer of at least
/// `minimum`; every other shape stays unknown rather than approximating.
fn normalized_bound_at_least(
    program: &TypedTrees,
    expression: ExpressionHandle,
    minimum: usize,
) -> bool {
    crate::normalize_immutable_integer_bound_to_usize(program, expression)
        .is_some_and(|bound| bound >= minimum)
}

fn names_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if path.symbol == parameter.symbol
                && path.head_symbol == parameter.symbol
                && program.expression_table.name_path_members(path.members).len() == 1
    )
}

fn parameter_is_slice(program: &TypedTrees, parameter: &StateParameter) -> bool {
    let mut reference = parameter.type_reference;
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return false,
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        Expression, NamePath, TableBinaryExpression, TableIndexedExpression, TableMemberExpression,
        TableRangeExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TableLocalData};

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn integer(program: &mut TypedTrees, value: i64) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    fn name(
        program: &mut TypedTrees,
        text: &'static str,
        symbol: SymbolHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated_static(text)],
                symbol,
                symbol,
            )))
    }

    fn length_of(program: &mut TypedTrees, receiver: ExpressionHandle) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: SymbolHandle::invalid(),
                member: Identifier::generated_static("len"),
                case_variant: None,
            }))
    }

    fn length_guard(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        operator: BinaryOperator,
        bound: ExpressionHandle,
    ) -> ExpressionHandle {
        let length = length_of(program, collection);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: length,
                operator,
                right: bound,
            }))
    }

    fn tail(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        start: ExpressionHandle,
    ) -> ExpressionHandle {
        let range = program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            }));
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index: range,
            }))
    }

    fn slice_parameter(program: &mut TypedTrees, symbol: SymbolHandle) -> StateParameter {
        let element = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: element,
            });
        StateParameter {
            symbol,
            name: Identifier::generated_static("items"),
            type_reference: slice,
            is_const: false,
            is_mutable: false,
            is_self: false,
        }
    }

    fn install_locals(
        program: &mut TypedTrees,
        locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
    ) {
        let element = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
        let mut machine = Machine::default();
        let mut state = State::default();
        for (symbol, name, initial_value, is_mutable) in locals {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    name: Identifier::generated_static(name),
                    type_reference: element,
                    initial_value,
                    is_mutable,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }

    #[test]
    fn exact_literal_tail_still_decreases() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let zero = integer(&mut program, 0);
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, zero);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn wider_literal_tail_decreases_under_a_matching_guard() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 2);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let two = integer(&mut program, 2);
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, two);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn shared_immutable_bound_tail_decreases_under_the_same_guard_bound() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let two = integer(&mut program, 2);
        install_locals(&mut program, [(symbol(2), "step", two, false)]);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let argument = tail(&mut program, items, step);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, step);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn mutable_and_short_bounds_do_not_fake_a_decrease() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let two = integer(&mut program, 2);
        install_locals(&mut program, [(symbol(2), "step", two, true)]);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let argument = tail(&mut program, items, step);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, step);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // A `2..` tail under `items.len >= 1` is not provably shorter: the
        // guard never establishes `items.len >= 2`, so the tail may not even
        // be a valid window, let alone a strict decrease.
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 2);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let one = integer(&mut program, 1);
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, one);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // `0..` is the whole slice: no decrease even under `len > 0`.
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 0);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let zero = integer(&mut program, 0);
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, zero);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }
}
