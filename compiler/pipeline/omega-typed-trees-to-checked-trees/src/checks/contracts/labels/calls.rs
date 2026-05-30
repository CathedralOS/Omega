use omega_core::symbols::SymbolHandle;

/// The reserved binder naming a call's return value inside an `ensures` clause.
pub(crate) const RESULT_BINDER: &str = "result";

/// Render the label of the value a call produces, used to substitute the
/// `result` binder of the callee's `ensures` clause into caller terms. This is
/// the call expression itself (`receiver.target(args)` or `target(args)`), so a
/// fact like `ensures result in String::Utf8` becomes a domain fact on the
/// concrete call result at the call site.
fn call_result_label(
    program: &omega_typed_trees::TypedTrees,
    call_site: &crate::CallSite<'_>,
) -> String {
    let argument_list = |arguments| {
        program
            .expression_table
            .expression_handles(arguments)
            .iter()
            .map(|argument| program.expression_table.display_name(*argument))
            .collect::<Vec<_>>()
            .join(", ")
    };

    match call_site {
        crate::CallSite::Statement(call) => {
            let arguments = argument_list(call.arguments);
            let receiver = omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(call.receiver),
                "::",
            );
            if receiver.is_empty() {
                format!("{}({arguments})", call.target)
            } else {
                format!("{receiver}.{}({arguments})", call.target)
            }
        }
        crate::CallSite::Expression(call) => {
            let arguments = argument_list(call.arguments);
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    program.expression_table.display_name(call.receiver),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        // A named transition target carries no single call result to bind.
        crate::CallSite::TransitionNamed(_) => RESULT_BINDER.to_owned(),
    }
}

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

            // The `result` binder refers to the value the call produces, not to
            // any parameter. An `ensures result in Domain` on the callee must
            // flow the domain fact onto the call's result value at the call site,
            // so substitute `result` with the call expression's own label. Only
            // a single-segment `result` that does not shadow a real parameter is
            // treated as the binder.
            if first_member == Some(RESULT_BINDER)
                && members.len() == 1
                && !program
                    .state_parameters(target_state)
                    .iter()
                    .any(|parameter| parameter.name.as_str() == RESULT_BINDER)
            {
                return call_result_label(program, call_site);
            }

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
