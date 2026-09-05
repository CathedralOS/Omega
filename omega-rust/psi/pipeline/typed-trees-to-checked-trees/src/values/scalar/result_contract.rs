//! Checked integer result predicates, separate from execution expressions.

use super::*;

/// The result-only contract namespace is separate from authored parameters.
/// The caller supplies a predicate from this machine's exact ensures clause.
pub(crate) fn lower_integer_result_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let entry = program.machine_states(machine).first()?;
    let primitive_type = program.primitive_type_reference(entry.return_type)?;
    if !is_integer(primitive_type)
        || program
            .state_parameters(entry)
            .iter()
            .any(|parameter| parameter.name.as_str() == "result")
    {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        let left = Box::new(lower_integer_result_predicate(
            program,
            operators,
            machine,
            binary.left,
        )?);
        let right = Box::new(lower_integer_result_predicate(
            program,
            operators,
            machine,
            binary.right,
        )?);
        return Some(if binary.operator == BinaryOperator::And {
            CheckedBooleanExpression::And { left, right }
        } else {
            CheckedBooleanExpression::Or { left, right }
        });
    }
    let is_result = |expression| {
        matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
        if matches!(program.expression_table.name_path_members(path.members), [name] if name.as_str() == "result"))
    };
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let operand_types =
        [binary.left, binary.right].map(|operand| is_result(operand).then_some(entry.return_type));
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &operand_types,
    ) {
        return None;
    }
    let mut result_seen = false;
    let mut operand = |expression| {
        if is_result(expression) {
            result_seen = true;
            return Some(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type,
            });
        }
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Integer(_)
        ) {
            return None;
        }
        lower_return_expression(
            program,
            operators,
            expression,
            &[],
            &[],
            &[],
            primitive_type,
            &[],
        )
    };
    let left = operand(binary.left)?;
    let right = operand(binary.right)?;
    if !result_seen {
        return None;
    }
    construct_integer_comparison(binary.operator, left, right)
}
