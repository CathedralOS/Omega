//! Guard facts require the current expression's builtin comparison meaning.

use super::*;
use language_core::OperatorSpelling;

pub(super) fn builtin_ordering(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match comparison.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ],
    )
}

pub(super) fn builtin_boolean_equality(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match comparison.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        _ => return false,
    };
    // Parser-generated arm comparisons are ordinary equality expressions too.
    // Neither their missing authored occurrence nor their Boolean literal is
    // evidence that a visible equality declaration has builtin meaning.
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ],
    )
}

fn operand_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // This is type lookup, not an immutable-value proof: mutable parameters
    // and locals keep ordinary evaluation-snapshot narrowing. Anonymous literal and
    // unresolved computed types remain wildcard candidates, never an assumed
    // copy of the other operand's carrier that could hide an overload.
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => builtin_bool_type_reference(program),
        ExpressionNode::Integer(_) => {
            crate::operators::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            crate::expression_types::named_value_type_reference(program, path)
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Call(_) => {
            declared_place_type_raw(program, machine, state, expression)
        }
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        // In particular, do not let declared_place_type_raw erase a Borrow
        // shell and incorrectly rule out a reference-typed operator candidate.
        _ => None,
    }?;
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

fn builtin_bool_type_reference(program: &TypedTrees) -> Option<TypeReferenceHandle> {
    // A Boolean literal has a fixed source type, independently of its sibling
    // expression. Reuse an actual reference to the exact compiler builtin atom;
    // do not manufacture a handle or identify a same-spelled user declaration.
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| {
            program.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::Bool)
        })?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

/// A recognized numeric bound must retain the meaning of the arithmetic
/// expression it consumes, independently of the body's eventual operator.
pub(super) fn builtin_arithmetic(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    builtin_arithmetic_node(program, machine, state, expression, binary)
        && folded_constant_is_builtin(program, machine, state, binary.left)
        && folded_constant_is_builtin(program, machine, state, binary.right)
}

fn builtin_arithmetic_node(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, binary.left),
            operand_type(program, machine, state, binary.right),
        ],
    )
}

/// `constant_integer_value` evaluates syntax, not selected declarations.
/// Check only the constant-shaped subtree it could fold; a nonconstant place
/// remains the responsibility of the calling relation recognizer.
pub(super) fn folded_constant_is_builtin(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    constant_subtree_meaning(program, machine, state, expression, 0).unwrap_or(true)
}

fn constant_subtree_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<bool> {
    if depth >= 128 {
        return Some(false);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(true),
        ExpressionNode::Borrow(borrow) => {
            constant_subtree_meaning(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Binary(binary) => {
            let left = constant_subtree_meaning(program, machine, state, binary.left, depth + 1)?;
            let right = constant_subtree_meaning(program, machine, state, binary.right, depth + 1)?;
            Some(
                left && right
                    && builtin_arithmetic_node(program, machine, state, expression, binary),
            )
        }
        _ => None,
    }
}
