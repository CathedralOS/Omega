use omega_core::symbols::SymbolHandle;

pub(crate) fn instantiate_call_contract_expression_label(
    program: &omega_typed_trees::TypedTrees,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> String {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| {
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        *value,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                binary.left,
            ),
            binary.operator.display_name(),
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                binary.right,
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                cast.value,
            ),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        *argument,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        call.receiver,
                    ),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        omega_typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                indexed.collection,
            ),
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                indexed.index,
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            match (range.start.is_valid(), range.end.is_valid()) {
                (true, true) => format!(
                    "{}..{}",
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        range.start,
                    ),
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        range.end,
                    ),
                ),
                (true, false) => format!(
                    "{}..",
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        range.start,
                    )
                ),
                (false, true) => format!(
                    "..{}",
                    instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        range.end,
                    )
                ),
                (false, false) => "..".to_owned(),
            }
        }
        omega_typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                member.receiver,
            ),
            member.member
        ),
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            format!(
                "mut {}",
                instantiate_call_contract_expression_label(
                    program,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    *inner,
                )
            )
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first_member = members.first().map(|member| member.as_str());
            let arguments = crate::call_site_argument_expressions(program, call_site);
            let mut argument_index = 0usize;

            for parameter in program.state_parameters(target_state) {
                let parameter_matches = first_member == Some(parameter.name.as_str())
                    || path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol;
                if parameter.is_self {
                    if parameter_matches {
                        return "self".to_owned();
                    }
                    continue;
                }

                let argument = arguments.get(argument_index).copied();
                argument_index = argument_index.saturating_add(1);
                if parameter_matches {
                    return argument
                        .map(|argument| program.expression_table.display_name(argument))
                        .unwrap_or_else(|| parameter.name.to_string());
                }
            }

            omega_typed_trees::expression::display_name_path(members, "::")
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        omega_typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
    }
}
