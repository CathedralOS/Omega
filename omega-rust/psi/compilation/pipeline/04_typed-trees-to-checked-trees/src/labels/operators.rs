use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::proposition::ProofSubstitutions;
use typed_trees::signature::StateParameter;

/// Render an operator contract expression with each formal parameter replaced
/// by its concrete operand label. This is shared by requires discharge and by
/// flow introduction of ensures facts so both sides use one canonical
/// caller-term representation. Callers supply the operand labels directly: a
/// spelled use renders its operands as written, while a named call names the
/// referent under a reference formal.
pub(crate) fn instantiate_operator_contract_expression_label(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    operand_labels: &[String],
    expression: ExpressionHandle,
) -> String {
    let instantiate = |expression: ExpressionHandle| {
        instantiate_operator_contract_expression_label(
            program,
            parameters,
            operand_labels,
            expression,
        )
    };

    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(saved) => format!(
            "$saved-subject#{}:{}",
            saved.capture.arena_index(),
            saved.capture.generation()
        ),
        ExpressionNode::Match(dispatch) => {
            let arms = program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| {
                    let pattern = match arm.pattern {
                        typed_trees::expression::MatchPattern::Value(pattern) => {
                            instantiate(pattern)
                        }
                        typed_trees::expression::MatchPattern::Wildcard => "_".to_owned(),
                        typed_trees::expression::MatchPattern::Destructure(_) => {
                            typed_trees::expression::display_match_pattern(
                                &program.expression_table,
                                arm.pattern,
                                &instantiate,
                            )
                        }
                    };
                    format!("{pattern} -> {}", instantiate(arm.value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {arms} }}", instantiate(dispatch.subject))
        }
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
            typed_trees::expression::display_name_path(
                program
                    .expression_table
                    .name_path_members(cast.target_label),
                "::",
            )
        ),
        ExpressionNode::Call(call) => typed_trees::expression::display_call_label(
            &program.expression_table,
            call,
            instantiate,
        ),
        ExpressionNode::Float(value) => value.to_string(),
        ExpressionNode::UndeterminedNaN(format) => {
            format!("<{} NaN without payload>", format.name())
        }
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
        ExpressionNode::Member(member) => format!(
            "{}{}{}",
            instantiate(member.receiver),
            language_core::PLACE_MEMBER_SEPARATOR,
            member.member
        ),
        ExpressionNode::Borrow(inner) => match inner.access {
            language_semantics::ReferenceAccess::Shared => instantiate(inner.target),
            language_semantics::ReferenceAccess::Mutable => {
                format!("mut {}", instantiate(inner.target))
            }
            language_semantics::ReferenceAccess::WriteOnly => {
                format!("write {}", instantiate(inner.target))
            }
        },
        ExpressionNode::Unary(unary) => format!(
            "{}{}",
            unary.operator.display_name(),
            instantiate(unary.operand)
        ),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first_member = members.first().map(|member| member.as_str());
            let operands_include_self = operand_labels.len() == parameters.len();
            let mut positional_operand_index = 0usize;

            for (parameter_index, parameter) in parameters.iter().enumerate() {
                let operand = if operands_include_self {
                    operand_labels.get(parameter_index)
                } else {
                    let operand = operand_labels.get(positional_operand_index);
                    positional_operand_index = positional_operand_index.saturating_add(1);
                    operand
                };

                let parameter_matches = first_member == Some(parameter.name.as_str())
                    || path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol;
                if parameter_matches {
                    return operand
                        .cloned()
                        .unwrap_or_else(|| parameter.name.to_string());
                }
            }

            program.render_proof_expression(expression, ProofSubstitutions::None)
        }
        ExpressionNode::StructLiteral(literal) => {
            program.render_proof_constructor_value(literal, instantiate)
        }
        ExpressionNode::String(value) => format!("{value:?}"),
        ExpressionNode::ZeroValue(type_reference) => format!(
            "zero_value<{}>()",
            program.display_type_reference(*type_reference)
        ),
    }
}

/// The Boolean clause's polarity decomposition: `!x` flips, `and`/`or`
/// distribute, every other leaf defers to the site's proof rule. Both the
/// operator-use discharge and the statement-entry discharge decompose the
/// clause tree identically before judging a leaf.
pub(crate) fn boolean_expression_has_polarity(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    polarity: bool,
    leaf: &mut impl FnMut(ExpressionHandle, bool) -> bool,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => *value == polarity,
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            boolean_expression_has_polarity(program, unary.operand, !polarity, leaf)
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            let left = boolean_expression_has_polarity(program, binary.left, polarity, leaf);
            let right = boolean_expression_has_polarity(program, binary.right, polarity, leaf);
            if (binary.operator == BinaryOperator::And) == polarity {
                left && right
            } else {
                left || right
            }
        }
        _ => leaf(expression, polarity),
    }
}
