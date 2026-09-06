//! Guard facts require the current expression's builtin comparison meaning.

use crate::places::declared_place_type_raw;
use language_core::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Preserve selected operator meaning before interpreting an expression as a
/// primitive bound. This grants no value, effect, or lifetime proof: callers
/// must still establish their own range and evaluation-snapshot obligations.
pub fn has_builtin_bound_expression_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    bound_subtree_meaning(program, machine, state, expression, 0)
}

/// Check a guard node whose Boolean children the caller decomposes and checks
/// separately. A true conjunction (or false disjunction) can contribute an
/// independent builtin fact even when another child selects an authored
/// operator. Boolean wrappers still owe their own exact equality meaning.
pub fn has_builtin_decomposed_guard_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) {
        match binary.operator {
            BinaryOperator::And | BinaryOperator::Or => return true,
            BinaryOperator::Equal | BinaryOperator::NotEqual
                if [binary.left, binary.right].into_iter().any(|operand| {
                    matches!(
                        program.expression_table.expression(operand),
                        ExpressionNode::Boolean(_)
                    )
                }) =>
            {
                return builtin_boolean_equality(program, machine, state, expression, binary);
            }
            _ => {}
        }
    }
    has_builtin_bound_expression_meaning(program, machine, state, expression)
}

fn bound_subtree_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if !expression.is_valid() || depth >= 128 {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let meaning = match binary.operator {
                BinaryOperator::Equal | BinaryOperator::NotEqual => {
                    builtin_boolean_equality(program, machine, state, expression, binary)
                }
                BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => {
                    builtin_ordering(program, machine, state, expression, binary)
                }
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo => {
                    builtin_arithmetic_node(program, machine, state, expression, binary)
                }
                // These operations have no overloadable operator spelling.
                BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight => true,
            };
            meaning
                && bound_subtree_meaning(program, machine, state, binary.left, depth + 1)
                && bound_subtree_meaning(program, machine, state, binary.right, depth + 1)
        }
        ExpressionNode::Borrow(borrow) => {
            bound_subtree_meaning(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Atomic(atomic) => {
            bound_subtree_meaning(program, machine, state, atomic.value, depth + 1)
        }
        ExpressionNode::Unary(unary) => {
            bound_subtree_meaning(program, machine, state, unary.operand, depth + 1)
        }
        ExpressionNode::Cast(cast) => {
            bound_subtree_meaning(program, machine, state, cast.value, depth + 1)
        }
        // Places and calls are symbolic leaves, not interpreted arithmetic.
        _ => true,
    }
}

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
