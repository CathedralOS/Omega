use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::signature::StateParameter;

/// Render an operator contract expression with each formal parameter replaced
/// by its concrete operand. This is shared by requires discharge and by flow
/// introduction of ensures facts so both sides use one canonical caller-term
/// representation.
pub(crate) fn instantiate_operator_contract_expression_label(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> String {
    let operand_labels = operands
        .iter()
        .map(|operand| program.expression_table.display_name(*operand))
        .collect::<Vec<_>>();
    instantiate_operator_contract_expression_label_with_labels(
        program,
        parameters,
        &operand_labels,
        expression,
    )
}

pub(crate) fn instantiate_operator_contract_expression_label_with_labels(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operand_labels: &[String],
    expression: ExpressionHandle,
) -> String {
    let instantiate = |expression: ExpressionHandle| {
        instantiate_operator_contract_expression_label_with_labels(
            program,
            parameters,
            operand_labels,
            expression,
        )
    };

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => format!(
            "atomic[{:?}]({})",
            atomic.ordering,
            instantiate(atomic.value)
        ),
        ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| instantiate(*value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate(binary.left),
            binary.operator.display_name(),
            instantiate(binary.right)
        ),
        ExpressionNode::Boolean(value) => value.to_string(),
        ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate(cast.value),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| instantiate(*argument))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate(call.receiver),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        ExpressionNode::Float(value) => value.to_string(),
        ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate(indexed.collection),
            instantiate(indexed.index)
        ),
        ExpressionNode::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
            (true, true) => format!("{}..{}", instantiate(range.start), instantiate(range.end)),
            (true, false) => format!("{}..", instantiate(range.start)),
            (false, true) => format!("..{}", instantiate(range.end)),
            (false, false) => "..".to_owned(),
        },
        ExpressionNode::Integer(value) => value.to_string(),
        ExpressionNode::Member(member) => {
            format!("{}.{}", instantiate(member.receiver), member.member)
        }
        ExpressionNode::Mutable(inner) => format!("mut {}", instantiate(*inner)),
        ExpressionNode::Unary(unary) => format!(
            "{}{}",
            unary.operator.display_name(),
            instantiate(unary.operand)
        ),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first_member = members.first().map(|member| member.as_str());
            let mut operand_index = 0usize;

            for parameter in parameters {
                if parameter.is_self {
                    continue;
                }
                let operand = operand_labels.get(operand_index);
                operand_index = operand_index.saturating_add(1);

                let parameter_matches = first_member == Some(parameter.name.as_str())
                    || path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol;
                if parameter_matches {
                    return operand
                        .cloned()
                        .unwrap_or_else(|| parameter.name.to_string());
                }
            }

            omega_typed_trees::expression::display_name_path(members, "::")
        }
        ExpressionNode::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
        ExpressionNode::String(value) => format!("{value:?}"),
    }
}
