use psi_symbols::SymbolHandle;

/// The reserved binder naming a call's return value inside an `ensures` clause.
pub(crate) const RESULT_BINDER: &str = "result";

/// Parameter source for an instantiated call contract. Ordinary calls use a
/// concrete state; static machine-parameter calls use the authored signature
/// directly while their generic body is checked.
pub(crate) trait ContractTargetParameters {
    fn contract_parameters<'program>(
        &'program self,
        program: &'program psi_typed_trees::TypedTrees,
    ) -> &'program [psi_typed_trees::signature::StateParameter];
}

impl ContractTargetParameters for psi_typed_trees::state::State {
    fn contract_parameters<'program>(
        &'program self,
        program: &'program psi_typed_trees::TypedTrees,
    ) -> &'program [psi_typed_trees::signature::StateParameter] {
        program.state_parameters(self)
    }
}

impl ContractTargetParameters for [psi_typed_trees::signature::StateParameter] {
    fn contract_parameters<'program>(
        &'program self,
        _program: &'program psi_typed_trees::TypedTrees,
    ) -> &'program [psi_typed_trees::signature::StateParameter] {
        self
    }
}

/// Render the label of the value a call produces, used to substitute the
/// `result` binder of the callee's `ensures` clause into caller terms. This is
/// the call expression itself (`receiver.target(args)` or `target(args)`), so a
/// fact like `ensures result in String::Utf8` becomes a domain fact on the
/// concrete call result at the call site.
fn call_result_label(
    program: &psi_typed_trees::TypedTrees,
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
            let receiver = psi_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(call.receiver),
                "::",
            );
            if receiver.is_empty() {
                format!("{}({arguments})", call.target)
            } else {
                format!("{receiver}.{}({arguments})", call.target)
            }
        }
        crate::CallSite::Expression { call, .. } => {
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
        crate::CallSite::TransitionNamed { .. } => RESULT_BINDER.to_owned(),
    }
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "call-site coordinates are deliberately threaded through recursive label rendering"
)]
pub(crate) fn instantiate_call_contract_expression_label(
    program: &psi_typed_trees::TypedTrees,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> String {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::Atomic(atomic) => format!(
            "atomic[{:?}]({})",
            atomic.ordering,
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                atomic.value,
            )
        ),
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
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
        psi_typed_trees::expression::ExpressionNode::Binary(binary) => format!(
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
        psi_typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        psi_typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                cast.value,
            ),
            psi_typed_trees::expression::display_name_path(
                program
                    .expression_table
                    .name_path_members(cast.target_label),
                "::",
            )
        ),
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
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
        psi_typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        psi_typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
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
        psi_typed_trees::expression::ExpressionNode::Range(range) => {
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
        psi_typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        psi_typed_trees::expression::ExpressionNode::Member(member) => format!(
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
        psi_typed_trees::expression::ExpressionNode::Borrow(inner) => {
            let target = instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                inner.target,
            );
            match inner.access {
                psi_language_semantics::ReferenceAccess::Shared => target,
                psi_language_semantics::ReferenceAccess::Mutable => format!("mut {target}"),
                psi_language_semantics::ReferenceAccess::WriteOnly => format!("write {target}"),
            }
        }
        psi_typed_trees::expression::ExpressionNode::Unary(unary) => format!(
            "{}{}",
            unary.operator.display_name(),
            instantiate_call_contract_expression_label(
                program,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                unary.operand,
            )
        ),
        psi_typed_trees::expression::ExpressionNode::Name(path) => {
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
                && !target_state
                    .contract_parameters(program)
                    .iter()
                    .any(|parameter| parameter.name.as_str() == RESULT_BINDER)
            {
                return call_result_label(program, call_site);
            }

            let arguments = crate::call_site_argument_expressions(program, call_site);
            let mut argument_index = 0usize;

            for parameter in target_state.contract_parameters(program) {
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

            psi_typed_trees::expression::display_name_path(members, "::")
        }
        psi_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        psi_typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
        psi_typed_trees::expression::ExpressionNode::ZeroValue(type_reference) => format!(
            "zero_value<{}>()",
            program.display_type_reference(*type_reference)
        ),
    }
}
