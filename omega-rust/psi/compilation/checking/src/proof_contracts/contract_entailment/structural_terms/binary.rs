//! Preserve scalar operation meaning while substituting structural operands.
//! This forms congruence terms; it neither evaluates arithmetic nor licenses
//! reassociation. The original declaration supplies operator and carrier facts.

use super::StructuralTerm;
use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Forms the congruence term for a selected builtin binary operation, with an
/// optional declaring `(machine, state)` fallback plus the expression's
/// expected (landed) result type. Pure literal operands carry no `Name`, so
/// `expression_scope` cannot anchor a scope walk, and fully anonymous operands
/// carry no resolvable result reference; callers that already know the
/// expression's owning state and expected type pass them through instead of
/// losing the term.
pub(crate) fn binary_term_in_scope(
    program: &TypedTrees,
    scope: Option<(&Machine, &State, TypeReferenceHandle)>,
    expression: ExpressionHandle,
    left: StructuralTerm,
    right: StructuralTerm,
) -> Option<StructuralTerm> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let (machine, state) = expression_scope(program, expression, 0)
        .and_then(|symbol| scoped_machine_state(program, symbol))
        .or_else(|| scope.map(|(machine, state, _)| (machine, state)))?;
    let expected = scope.map(|(_, _, expected)| expected);
    let references = [binary.left, binary.right]
        .map(|operand| crate::expression_result_type_reference(program, machine, state, operand));
    let spelling = match binary.operator {
        BinaryOperator::Equal => Some(OperatorSpelling::Equal),
        BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
        BinaryOperator::Less => Some(OperatorSpelling::Less),
        BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
        BinaryOperator::Greater => Some(OperatorSpelling::Greater),
        BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
        BinaryOperator::Add => Some(OperatorSpelling::Add),
        BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
        BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
        BinaryOperator::Divide => Some(OperatorSpelling::Divide),
        BinaryOperator::Modulo => Some(OperatorSpelling::Modulo),
        BinaryOperator::CaseMembership => return None,
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    };
    if let Some(spelling) = spelling
        && !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &references,
        )
    {
        return None;
    }
    let parent_reference =
        || crate::expression_result_type_reference(program, machine, state, expression);
    let reference = |index: usize| {
        references[index]
            .or_else(|| {
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then_some(references[1 - index])
                .flatten()
            })
            .or_else(|| {
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then(parent_reference)
                .flatten()
            })
            .or_else(|| {
                // Fully anonymous operand trees inherit the expression's
                // expected (landed) type -- the same resolution integer landing
                // applies when the operation is checked.
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then_some(expected)
                .flatten()
            })
    };
    let meaning = [reference(0)?, reference(1)?].map(|reference| {
        Some((
            program.primitive_type_reference(reference)?,
            program.arithmetic_domain_for_type_reference(reference),
        ))
    });
    Some(StructuralTerm::ScalarBinary {
        operator: binary.operator,
        meaning: [meaning[0]?, meaning[1]?],
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// Walk an anchored name's symbol upward to its declaring machine and state.
fn scoped_machine_state<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&'program Machine, &'program State)> {
    let mut owner = symbol;
    let mut visited = Vec::new();
    loop {
        if !owner.is_valid() || visited.contains(&owner) {
            return None;
        }
        visited.push(owner);
        if let Some(selected) = program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == owner)
                .map(|state| (machine, state))
        }) {
            return Some(selected);
        }
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == owner)
        {
            return Some((machine, program.machine_states(machine).first()?));
        }
        owner = program.symbols.get(owner).parent;
    }
}

fn expression_scope(
    program: &TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 32 {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => path.symbol.is_valid().then_some(path.symbol),
        ExpressionNode::Binary(binary) => expression_scope(program, binary.left, depth + 1)
            .or_else(|| expression_scope(program, binary.right, depth + 1)),
        ExpressionNode::Member(member) => expression_scope(program, member.receiver, depth + 1),
        ExpressionNode::Cast(cast) => expression_scope(program, cast.value, depth + 1),
        _ => None,
    }
}
