//! Retarget a bound source requirement to its exact executable application.
//! The original receiver occurrence supplies its outer scope; neither selected
//! maps nor equal ABI carriers can supply a missing or different application.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::StaticMachineArgument, types::TypeReferenceHandle};

fn instance(
    program: &TypedTrees,
    lexical_owner: SymbolHandle,
    target: SymbolHandle,
    receiver: Option<TypeReferenceHandle>,
    arguments: &[StaticMachineArgument],
) -> SymbolHandle {
    let template = program
        .closed_requirement_application(target)
        .map_or(target, |application| application.template);
    let Some((owner, _)) = program.trait_machine_signature(template) else {
        return SymbolHandle::invalid();
    };
    let scope = match receiver.and_then(|reference| {
        typed_trees::service::classify_exact_service_parameter_carrier(
            program,
            typed_trees::service::ServiceCarrierOccurrence {
                type_reference: reference,
                lexical_owner,
                enclosing_data_applications: &[],
            },
        )
        .ok()
        .flatten()
    }) {
        Some(carrier) => carrier
            .carrier
            .application(program, lexical_owner, &[])
            .and_then(|scope| scope.declaring_scope(program, owner.symbol)),
        None if program.trait_type_parameters(owner).is_empty()
            && owner.lifetime_parameters.is_empty() =>
        {
            Some(typed_trees::trait_definition::AppliedTraitScope::abstract_scope(owner))
        }
        None => None,
    };
    let Some(scope) = scope else {
        return SymbolHandle::invalid();
    };
    instance_in_scope(program, template, &scope, arguments)
}

fn instance_in_scope(
    program: &TypedTrees,
    template: SymbolHandle,
    scope: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    arguments: &[StaticMachineArgument],
) -> SymbolHandle {
    let Some(tuple) = arguments
        .iter()
        .map(|argument| program.static_const_argument_identity(argument))
        .collect::<Option<Vec<_>>>()
    else {
        return SymbolHandle::invalid();
    };
    let mut matches = program
        .closed_requirement_applications()
        .filter(|application| {
            application.template == template
                && program
                    .requirement_application_matches_scope(application.signature.symbol, scope)
                && program
                    .expression_table
                    .static_machine_application_arguments(application.application)
                    .iter()
                    .map(|argument| program.static_const_argument_identity(argument))
                    .collect::<Option<Vec<_>>>()
                    .as_ref()
                    == Some(&tuple)
        });
    let Some(application) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        return SymbolHandle::invalid();
    }
    application.signature.symbol
}

/// The executable target remains the chosen provider. Only its public contract
/// coordinate is applied, through the same committed map and exact scope that
/// call admission checks. A provider's stronger private contract is irrelevant.
fn dispatch_instance(
    program: &TypedTrees,
    owner: SymbolHandle,
    dispatch: &typed_trees::typed_trees::StaticRequirementDispatch,
    arguments: &[StaticMachineArgument],
) -> SymbolHandle {
    let applications = typed_trees::closed_conformance_applications_of(
        &program.machine_specializations,
        owner,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return SymbolHandle::invalid();
    };
    let Some(scope) = application.trait_scope.scope(program) else {
        return SymbolHandle::invalid();
    };
    let template = program
        .closed_requirement_application(dispatch.requirement)
        .map_or(dispatch.requirement, |closed| closed.template);
    instance_in_scope(program, template, &scope, arguments)
}

pub(super) fn retarget(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut expression_updates = Vec::new();
    let mut expression_dispatch_updates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut statement_updates = Vec::new();
    let mut statement_dispatch_updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (position, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    if let Some(dispatch) = &call.static_requirement_dispatch {
                        let selected = dispatch_instance(
                            program,
                            machine.symbol,
                            dispatch,
                            &call.machine_arguments,
                        );
                        if selected.is_valid() {
                            statement_dispatch_updates
                                .push((state.statement_nodes.handle_at(position as u32), selected));
                        }
                    }
                    let receiver = validation::projected_statement_receiver_type_reference(
                        program, state, position, call,
                    );
                    let selected = instance(
                        program,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        &call.machine_arguments,
                    );
                    if selected.is_valid() {
                        statement_updates
                            .push((state.statement_nodes.handle_at(position as u32), selected));
                    }
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let Some(call) = program.expression_table.call_node(expression) else {
                        continue;
                    };
                    if let Some(dispatch) = &call.static_requirement_dispatch {
                        let selected = dispatch_instance(
                            program,
                            machine.symbol,
                            dispatch,
                            &call.machine_arguments,
                        );
                        if selected.is_valid() {
                            if let Some((_, prior)) = expression_dispatch_updates
                                .iter()
                                .find(|(handle, _)| *handle == expression)
                            {
                                if *prior != selected {
                                    diagnostics.push(Diagnostic::error("one evidence expression occurs under disagreeing exact trait applications"));
                                }
                            } else {
                                expression_dispatch_updates.push((expression, selected));
                            }
                        }
                    }
                    let receiver = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        position,
                        call.receiver,
                    );
                    let selected = instance(
                        program,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        &call.machine_arguments,
                    );
                    if !selected.is_valid() {
                        continue;
                    }
                    if let Some((_, prior)) = expression_updates
                        .iter()
                        .find(|(handle, _)| *handle == expression)
                    {
                        if *prior != selected {
                            diagnostics.push(Diagnostic::error("one requirement expression occurs under disagreeing exact trait applications"));
                        }
                    } else {
                        expression_updates.push((expression, selected));
                    }
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (handle, selected) in expression_updates {
        if let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(handle)
        {
            call.target_symbol = selected;
        }
    }
    for (handle, selected) in statement_updates {
        if let typed_trees::statement::StatementNode::Call(call) =
            program.statement_table.statement_mut(handle)
        {
            call.target_symbol = selected;
        }
    }
    for (handle, selected) in expression_dispatch_updates {
        if let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(handle)
            && let Some(dispatch) = &mut call.static_requirement_dispatch
        {
            dispatch.contract_signature = selected;
        }
    }
    for (handle, selected) in statement_dispatch_updates {
        if let typed_trees::statement::StatementNode::Call(call) =
            program.statement_table.statement_mut(handle)
            && let Some(dispatch) = &mut call.static_requirement_dispatch
        {
            dispatch.contract_signature = selected;
        }
    }
    Ok(())
}
