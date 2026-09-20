//! Preserve scalar operation meaning while substituting structural operands.
//! This forms congruence terms; it neither evaluates arithmetic nor licenses
//! reassociation. The original declaration supplies operator and carrier facts.

use super::StructuralTerm;
use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(crate) fn binary_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
    left: StructuralTerm,
    right: StructuralTerm,
) -> Option<StructuralTerm> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let symbol = expression_scope(program, expression, 0)?;
    let mut owner = symbol;
    let mut visited = Vec::new();
    let (machine, state) = loop {
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
            break selected;
        }
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == owner)
        {
            break (machine, program.machine_states(machine).first()?);
        }
        owner = program.symbols.get(owner).parent;
    };
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
    let reference = |index: usize| {
        references[index].or_else(|| {
            crate::value_custody::literals::has_anonymous_numeric_results(
                program,
                [binary.left, binary.right][index],
            )
            .then_some(references[1 - index])
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
