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
    // and locals keep ordinary evaluation-snapshot narrowing. Numeric literal
    // and unresolved computed types remain wildcard candidates, never an
    // assumed copy of the other operand's carrier that could hide an overload.
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => builtin_bool_type_reference(program),
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
