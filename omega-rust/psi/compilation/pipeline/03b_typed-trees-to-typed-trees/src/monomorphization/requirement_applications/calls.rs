//! Retarget a source requirement only after retaining its complete occurrence
//! application. The original receiver supplies the selected outer telescope;
//! the original call supplies member operands and lexical/captured custody.
//! The same TypedTrees matcher serves this producer and downstream call lookup.
//! No selected provider or equal ABI carrier supplies a missing application.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::static_applications::{
    StaticApplicationView, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineBindingValue, static_application_identity,
};
use typed_trees::{TypedTrees, expression::StaticMachineArgument};

struct SourceCall {
    target: SymbolHandle,
    receiver: Option<validation::CanonicalPlaceTypeApplication>,
    value_arguments: Vec<typed_trees::expression::ExpressionHandle>,
    state: SymbolHandle,
    position: usize,
    arguments: Vec<StaticMachineArgument>,
    application: StaticMachineApplicationHandle,
    lexical_owner: SymbolHandle,
}

fn source_type_application(
    program: &TypedTrees,
    state: SymbolHandle,
    position: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<validation::CanonicalPlaceTypeApplication> {
    let place =
        validation::canonical_place_from_expression_in_state(program, state, position, expression)?;
    let (reference, enclosing_data_applications) =
        validation::canonical_place_source_type_application(program, state, position, &place)?;
    Some(validation::CanonicalPlaceTypeApplication {
        reference,
        enclosing_data_applications,
    })
}

fn source_input_application(
    program: &mut TypedTrees,
    source: &SourceCall,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<validation::borrow_contracts::InvocationInputTypeApplication> {
    let machine = program.machine_by_symbol(source.lexical_owner)?;
    let state = validation::find_state_in_machine(program, source.lexical_owner, source.state)?;
    let result = validation::expression_result_type_reference(program, machine, state, expression);
    let original = source_type_application(program, source.state, source.position, expression);
    let (mut reference, enclosing_data_applications) = match original {
        Some(original) => (original.reference, original.enclosing_data_applications),
        // A computed value owns its existing result type. It has no enclosing
        // storage telescope to borrow from another argument or receiver.
        None => {
            let place = validation::canonical_place_from_expression_in_state(
                program,
                source.state,
                source.position,
                expression,
            );
            match place {
                Some(place)
                    if place.root.as_expression() == Some(expression)
                        && place.segments.is_empty() =>
                {
                    (
                        result.unwrap_or(typed_trees::types::TypeReferenceHandle::invalid()),
                        Vec::new(),
                    )
                }
                // A missing semantic storage application cannot be replaced by
                // its prepared result or an empty supplying telescope.
                _ => (
                    typed_trees::types::TypeReferenceHandle::invalid(),
                    Vec::new(),
                ),
            }
        }
    };
    if reference.is_valid()
        && program
            .expression_table
            .borrow_expression(expression)
            .is_some()
    {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if !matches!(
            program.type_reference_table.type_reference(carrier),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            // Borrow formation supplies this actual outer shell. Preserve its
            // original semantic inner graph instead of combining prepared
            // storage fields with their original substitution frames. No loan
            // or lifetime is inferred from the receiving parameter.
            let mut shell = program.type_reference_table.type_reference(result?).clone();
            let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = &mut shell
            else {
                return None;
            };
            *referee = reference;
            reference = program.type_reference_table.insert(shell);
        }
    }
    Some(
        validation::borrow_contracts::InvocationInputTypeApplication {
            reference,
            enclosing_data_applications,
        },
    )
}

fn binding(
    program: &mut TypedTrees,
    parameter: &TypeParameter,
    argument: &StaticMachineArgument,
) -> Option<StaticMachineBinding> {
    let value = match parameter.kind {
        TypeParameterKind::Type => {
            StaticMachineBindingValue::Type(program.static_operand_type_reference(argument).ok()?)
        }
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
            StaticMachineBindingValue::Const(program.static_operand_type_reference(argument).ok()?)
        }
        TypeParameterKind::Machine { .. } => StaticMachineBindingValue::Machine(argument.clone()),
        TypeParameterKind::Implementation => {
            StaticMachineBindingValue::Conformance(argument.clone())
        }
        TypeParameterKind::Proposition { .. } => return None,
    };
    Some(StaticMachineBinding {
        parameter: parameter.symbol,
        value,
    })
}

fn source_application(
    program: &mut TypedTrees,
    source: &SourceCall,
) -> Option<(SymbolHandle, StaticMachineApplicationHandle)> {
    let template = program
        .closed_requirement_application(source.target)
        .map_or(source.target, |row| row.template);
    if !program
        .closed_requirement_applications()
        .any(|row| row.template == template)
    {
        return None;
    }
    // Original demand/header preparation may already retain the whole tuple.
    // Reuse that authentic occurrence instead of deriving it a second time.
    if program
        .expression_table
        .static_machine_application_is_valid(source.application)
        && let Ok(actual) = StaticApplicationView::new(program, source.application)
        && actual.declaration().symbol() == template
        && actual.application().registration_operation == template
        && actual.application().lexical_owner.is_valid()
    {
        return Some((template, source.application));
    }
    let (owner, signature) = program.trait_machine_signature(template)?;
    let owner_symbol = owner.symbol;
    let signature = signature.clone();
    let name = signature.name.clone();
    let local_parameters = program.state_signature_type_parameters(&signature).to_vec();
    let previous_bindings = if program
        .expression_table
        .static_machine_application_is_valid(source.application)
    {
        program
            .expression_table
            .static_machine_application_bindings(source.application)
            .to_vec()
    } else {
        Vec::new()
    };
    let local_bindings = if source.arguments.len() == local_parameters.len() {
        local_parameters
            .iter()
            .zip(&source.arguments)
            .map(|(parameter, argument)| binding(program, parameter, argument))
            .collect::<Option<Vec<_>>>()?
    } else if source.arguments.is_empty() {
        local_parameters
            .iter()
            .map(|parameter| {
                let mut rows = previous_bindings
                    .iter()
                    .filter(|row| row.parameter == parameter.symbol);
                let row = rows.next()?;
                if rows.next().is_some() {
                    return None;
                }
                Some(row.clone())
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        return None;
    };
    let inputs = if signature.lifetime_parameters.is_empty() {
        Vec::new()
    } else {
        source
            .value_arguments
            .iter()
            .map(|argument| source_input_application(program, source, *argument))
            .collect::<Option<Vec<_>>>()?
    };
    let owner = program.trait_definition_by_symbol(owner_symbol)?;
    let scope = match source.receiver.as_ref().and_then(|receiver| {
        typed_trees::service::classify_exact_service_parameter_carrier(
            program,
            typed_trees::service::ServiceCarrierOccurrence {
                type_reference: receiver.reference,
                lexical_owner: source.lexical_owner,
                enclosing_data_applications: &receiver.enclosing_data_applications,
            },
        )
        .ok()
        .flatten()
    }) {
        Some(carrier) => carrier.carrier.application(
            program,
            source.lexical_owner,
            &source.receiver.as_ref()?.enclosing_data_applications,
        )?,
        None if program.trait_type_parameters(owner).is_empty()
            && owner.lifetime_parameters.is_empty() =>
        {
            typed_trees::trait_definition::AppliedTraitScope::abstract_scope(owner)
        }
        None => return None,
    };
    let declaring = scope.declaring_scope(program, owner.symbol)?;
    let mut lifetimes = scope.captured_lifetime_arguments(program)?;
    if !signature.lifetime_parameters.is_empty() {
        let member_arguments = local_bindings
            .iter()
            .filter_map(|binding| match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    Some((binding.parameter, *reference))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        lifetimes.extend(
            validation::borrow_contracts::infer_requirement_input_lifetime_application(
                program,
                &signature,
                &declaring,
                &member_arguments,
                source.lexical_owner,
                source.application,
                &inputs,
            )
            .ok()?,
        );
    }
    let selected_interface = scope.declaration.symbol;
    let outer_parameters = program.trait_type_parameters(scope.declaration).to_vec();
    let environment = scope.environment();
    let previous = if program
        .expression_table
        .static_machine_application_is_valid(source.application)
    {
        let row = program
            .expression_table
            .static_machine_application(source.application)
            .clone();
        if row.registration_operation != template
            || (row.binding_declaration.is_valid() && row.binding_declaration != template)
            || (row.selected_interface.is_valid() && row.selected_interface != selected_interface)
            || !row.lexical_owner.is_valid()
            || row.type_identity_owner(program).is_none()
        {
            return None;
        }
        Some(row)
    } else if source.application.is_valid() {
        return None;
    } else {
        None
    };
    let outer_arguments = program
        .materialize_trait_application_arguments(&environment)
        .ok()?;
    program
        .require_closed_application_operands(&outer_arguments)
        .ok()?;
    let mut bindings = Vec::new();
    for (parameter, reference) in outer_parameters.iter().zip(&outer_arguments) {
        let value = match parameter.kind {
            TypeParameterKind::Type => StaticMachineBindingValue::Type(*reference),
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                StaticMachineBindingValue::Const(*reference)
            }
            TypeParameterKind::Machine { .. } => StaticMachineBindingValue::Machine(
                program
                    .static_operand_from_type_reference(*reference)
                    .ok()?,
            ),
            TypeParameterKind::Implementation => StaticMachineBindingValue::Conformance(
                program
                    .static_operand_from_type_reference(*reference)
                    .ok()?,
            ),
            TypeParameterKind::Proposition { .. } => return None,
        };
        bindings.push(StaticMachineBinding {
            parameter: parameter.symbol,
            value,
        });
    }
    bindings.extend(local_bindings);
    // Evidence remains original occurrence data. No bound is inferred from a
    // selected map or the closed executable row; view admission rejects extras.
    bindings.extend(
        previous_bindings
            .iter()
            .filter(|row| {
                !outer_parameters
                    .iter()
                    .chain(&local_parameters)
                    .any(|parameter| parameter.symbol == row.parameter)
            })
            .cloned(),
    );
    let arguments = previous.as_ref().map_or_else(
        || source.arguments.clone(),
        |_| {
            program
                .expression_table
                .static_machine_application_arguments(source.application)
                .to_vec()
        },
    );
    // This producer obtains outer operands from the original receiver. Only
    // the member operands occupy the retained authored list; carry their
    // exact original binder coordinates instead of guessing from tuple shape.
    if arguments.len() > local_parameters.len() {
        return None;
    }
    let argument_parameters = local_parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, _)| parameter.symbol)
        .collect::<Vec<_>>();
    if previous.is_some() {
        // A retained partial lane constrains this same original occurrence;
        // new input inference may fill missing slots but never overwrite it.
        for previous in program
            .expression_table
            .static_machine_application_lifetimes(source.application)
        {
            let mut matches = lifetimes
                .iter()
                .filter(|actual| actual.parameter == previous.parameter);
            if matches.next() != Some(previous) || matches.next().is_some() {
                return None;
            }
        }
    }
    let selected_machines = previous.as_ref().map_or_else(Vec::new, |_| {
        program
            .expression_table
            .static_machine_application_selected_machines(source.application)
            .to_vec()
    });
    let application = program
        .expression_table
        .retain_static_machine_application(template, name, &arguments);
    let row = program
        .expression_table
        .static_machine_application_mut(application);
    row.selected_interface = selected_interface;
    row.binding_declaration = template;
    if let Some(previous) = previous {
        row.lexical_owner = previous.lexical_owner;
        row.captured_lexical_scope = previous.captured_lexical_scope;
    } else {
        row.lexical_owner = source.lexical_owner;
    }
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    program
        .expression_table
        .set_static_machine_application_argument_parameters(application, &argument_parameters);
    program
        .expression_table
        .set_static_machine_application_lifetimes(application, &lifetimes);
    program
        .expression_table
        .set_static_machine_application_selected_machines(application, &selected_machines);
    StaticApplicationView::new(program, application).ok()?;
    Some((template, application))
}

fn instance(
    program: &TypedTrees,
    template: SymbolHandle,
    application: StaticMachineApplicationHandle,
) -> SymbolHandle {
    let mut matches = program.closed_requirement_applications().filter(|row| {
        row.template == template
            && program
                .closed_requirement_application_matches_call(row.signature.symbol, application)
    });
    let Some(row) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        return SymbolHandle::invalid();
    }
    row.signature.symbol
}

pub(super) fn retarget(program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
    let mut expression_sources = Vec::new();
    let mut statement_sources = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (position, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    statement_sources.push((
                        state.statement_nodes.handle_at(position as u32),
                        SourceCall {
                            target: call.target_symbol,
                            receiver:
                                validation::projected_statement_receiver_source_type_application(
                                    program, state, position, call,
                                ),
                            value_arguments: program
                                .statement_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                            state: state.symbol,
                            position,
                            arguments: call.machine_arguments.to_vec(),
                            application: call.authored_machine_application,
                            lexical_owner: machine.symbol,
                        },
                    ));
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
                    expression_sources.push((
                        expression,
                        SourceCall {
                            target: call.target_symbol,
                            receiver: source_type_application(
                                program,
                                state.symbol,
                                position,
                                call.receiver,
                            ),
                            value_arguments: program
                                .expression_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                            state: state.symbol,
                            position,
                            arguments: call.machine_arguments.to_vec(),
                            application: call.authored_machine_application,
                            lexical_owner: machine.symbol,
                        },
                    ));
                }
            }
        }
    }
    let mut expression_updates = Vec::new();
    let mut statement_updates = Vec::new();
    let mut diagnostics = Vec::new();
    for (handle, source) in expression_sources {
        let Some((template, application)) = source_application(program, &source) else {
            continue;
        };
        let selected = instance(program, template, application);
        if !selected.is_valid() {
            continue;
        }
        if let Some((_, prior, prior_application, prior_owner)) = expression_updates
            .iter()
            .find(|(expression, _, _, _)| *expression == handle)
        {
            let agrees = matches!((
                static_application_identity(program, *prior_application),
                static_application_identity(program, application),
            ), (Ok(prior), Ok(actual)) if prior == actual);
            if *prior != selected || *prior_owner != source.lexical_owner || !agrees {
                diagnostics.push(Diagnostic::error(
                    "one requirement expression occurs under disagreeing exact source applications",
                ));
            }
        } else {
            expression_updates.push((handle, selected, application, source.lexical_owner));
        }
    }
    for (handle, source) in statement_sources {
        let Some((template, application)) = source_application(program, &source) else {
            continue;
        };
        let selected = instance(program, template, application);
        if selected.is_valid() {
            statement_updates.push((handle, selected, application));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut changed = 0usize;
    for (handle, selected, application, _) in expression_updates {
        if let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(handle)
        {
            if call.target_symbol != selected || call.authored_machine_application != application {
                call.target_symbol = selected;
                call.authored_machine_application = application;
                changed += 1;
            }
        }
    }
    for (handle, selected, application) in statement_updates {
        if let typed_trees::statement::StatementNode::Call(call) =
            program.statement_table.statement_mut(handle)
        {
            if call.target_symbol != selected || call.authored_machine_application != application {
                call.target_symbol = selected;
                call.authored_machine_application = application;
                changed += 1;
            }
        }
    }
    Ok(changed)
}
