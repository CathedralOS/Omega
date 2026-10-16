use symbols::SymbolHandle;
use typed_trees::proposition::ProofSubstitutions;

/// The reserved binder naming a call's return value inside an `ensures` clause.
pub(crate) const RESULT_BINDER: &str = "result";

/// Parameter source for an instantiated call contract. Ordinary calls use a
/// concrete state; static machine-parameter calls use the authored signature
/// directly while their generic body is checked.
pub(crate) trait ContractTargetParameters {
    fn contract_parameters<'program>(
        &'program self,
        program: &'program dyn ::validation::CheckingInput,
    ) -> &'program [typed_trees::signature::StateParameter];
}

impl ContractTargetParameters for typed_trees::state::State {
    fn contract_parameters<'program>(
        &'program self,
        program: &'program dyn ::validation::CheckingInput,
    ) -> &'program [typed_trees::signature::StateParameter] {
        program.state_parameters(self)
    }
}

impl ContractTargetParameters for [typed_trees::signature::StateParameter] {
    fn contract_parameters<'program>(
        &'program self,
        _program: &'program dyn ::validation::CheckingInput,
    ) -> &'program [typed_trees::signature::StateParameter] {
        self
    }
}

/// Render the label of the value a call produces, used to substitute the
/// `result` binder of the callee's `ensures` clause into caller terms. This is
/// the call expression itself (`receiver.target(args)` or `target(args)`), so a
/// fact like `ensures result in String::Utf8` becomes a domain fact on the
/// concrete call result at the call site.
fn call_result_label(
    program: &dyn ::validation::CheckingInput,
    call_site: &validation::CallSite<'_>,
) -> String {
    let argument_list = |arguments| {
        program
            .expression_table
            .expression_handles(arguments)
            .iter()
            .map(|argument| program.render_proof_expression(*argument, ProofSubstitutions::None))
            .collect::<Vec<_>>()
            .join(", ")
    };

    match call_site {
        validation::CallSite::Statement(call) => {
            let arguments = argument_list(call.arguments);
            let receiver = typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(call.receiver),
                "::",
            );
            if receiver.is_empty() {
                format!("{}({arguments})", call.target)
            } else {
                format!("{receiver}.{}({arguments})", call.target)
            }
        }
        validation::CallSite::Expression { call, .. } => {
            let arguments = argument_list(call.arguments);
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    program.render_proof_expression(call.receiver, ProofSubstitutions::None),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        // A named transition target carries no single call result to bind.
        validation::CallSite::TransitionNamed { .. } => RESULT_BINDER.to_owned(),
    }
}

#[allow(
    clippy::only_used_in_recursion,
    reason = "call-site coordinates are deliberately threaded through recursive label rendering"
)]
pub(crate) fn instantiate_call_contract_expression_label(
    program: &dyn ::validation::CheckingInput,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: typed_trees::expression::ExpressionHandle,
) -> String {
    let render = |value: typed_trees::expression::ExpressionHandle| {
        instantiate_call_contract_expression_label(
            program,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            value,
        )
    };
    let name_label = |path: &typed_trees::expression::TableNamePath| -> String {
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

        let arguments = validation::call_site_argument_expressions(program, call_site);
        let mut argument_index = 0usize;

        for parameter in target_state.contract_parameters(program) {
            let parameter_matches = first_member == Some(parameter.name.as_str())
                || path.head_symbol == parameter.symbol
                || path.symbol == parameter.symbol;

            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            if parameter_matches {
                return argument
                    .map(|argument| {
                        // A reference formal used as a predicate value reads
                        // its referent. The borrow at the call site supplies
                        // access, not an extra operator in that predicate.
                        let argument = match (
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            program.expression_table.expression(argument),
                        ) {
                            (
                                typed_trees::types::TypeReferenceNode::Reference { .. },
                                typed_trees::expression::ExpressionNode::Borrow(borrow),
                            ) => borrow.target,
                            _ => argument,
                        };
                        program.render_proof_expression(argument, ProofSubstitutions::None)
                    })
                    .unwrap_or_else(|| parameter.name.to_string());
            }
        }

        // A `const`/`Value` binder of the target names the machine argument
        // this call supplies for it, positionally (`bounded<K>` turns the
        // callee's `Count <= 10` into the caller's `K <= 10`). A generic
        // caller forwarding its own binder still owes that fact, which its
        // own requires contract establishes.
        let machine_arguments = call_site_machine_arguments(call_site);
        if members.len() == 1 && !machine_arguments.is_empty() {
            let type_parameters = validation::call_target_type_parameters(
                program,
                call_site_target_symbol(call_site),
            );
            for (position, parameter) in type_parameters.enumerate() {
                let binder_matches = matches!(
                    &parameter.kind,
                    typed_trees::data::TypeParameterKind::Const { .. }
                        | typed_trees::data::TypeParameterKind::Value { .. }
                ) && (path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol
                    || first_member == Some(parameter.name.as_str()));
                if binder_matches && let Some(argument) = machine_arguments.get(position) {
                    return argument.display_name();
                }
            }
        }

        program.render_proof_expression(expression, ProofSubstitutions::None)
    };
    super::render::expression_label(
        program,
        program.expression_table.expression(expression),
        &render,
        &name_label,
        &|literal| program.render_proof_constructor_value(literal, |value| render(value)),
    )
}

/// The static generic arguments authored on the call (`bounded<K>`), empty
/// for a named transition which carries none.
fn call_site_machine_arguments<'program>(
    call_site: &'program validation::CallSite<'program>,
) -> &'program [typed_trees::expression::StaticMachineArgument] {
    match call_site {
        validation::CallSite::Statement(call) => &call.machine_arguments,
        validation::CallSite::Expression { call, .. } => &call.machine_arguments,
        validation::CallSite::TransitionNamed { .. } => &[],
    }
}

/// The state symbol the call targets, used to reach the callee's declared
/// generic telescope for binder instantiation.
fn call_site_target_symbol(call_site: &validation::CallSite<'_>) -> SymbolHandle {
    match call_site {
        validation::CallSite::Statement(call) => call.target_symbol,
        validation::CallSite::Expression { call, .. } => call.target_symbol,
        validation::CallSite::TransitionNamed { path, .. } => path.symbol,
    }
}
