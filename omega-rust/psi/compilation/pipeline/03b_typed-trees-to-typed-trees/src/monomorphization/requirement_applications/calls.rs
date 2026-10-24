//! Retarget a bound source requirement to its exact executable application.
//! The original qualified requirement occurrence supplies its outer scope; neither selected
//! maps nor equal ABI carriers can supply a missing or different application.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees, expression::StaticMachineArgument, references::StaticArgumentCall,
    trait_definition::AppliedTraitScope, types::TypeReferenceHandle,
};

mod invocation_binding_views;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct InvocationTuple {
    pub owner: SymbolHandle,
    pub arguments: Vec<StaticMachineArgument>,
    pub lifetimes: Vec<typed_trees::static_applications::StaticMachineLifetimeBinding>,
}

fn occurrence(
    program: &TypedTrees,
    owner: SymbolHandle,
    call: StaticArgumentCall<'_>,
) -> InvocationTuple {
    let application = call.application();
    let arguments = invocation_binding_views::complete_arguments(program, owner, call)
        .unwrap_or_else(|| call.arguments(program).to_vec());
    if application.is_valid() {
        InvocationTuple {
            owner: program
                .expression_table
                .static_machine_application(application)
                .lexical_owner,
            arguments,
            lifetimes: program
                .expression_table
                .static_machine_application_lifetimes(application)
                .to_vec(),
        }
    } else {
        InvocationTuple {
            owner,
            arguments,
            lifetimes: Vec::new(),
        }
    }
}

fn instance(
    program: &TypedTrees,
    lexical_owner: SymbolHandle,
    target: SymbolHandle,
    receiver: Option<AppliedTraitScope<'_>>,
    application: TypeReferenceHandle,
    invocation: &InvocationTuple,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let Some((template, scope)) =
        source_scope(program, lexical_owner, target, receiver, application)?
    else {
        return Ok(SymbolHandle::invalid());
    };
    Ok(instance_in_scope(program, template, &scope, invocation))
}

fn source_scope<'p>(
    program: &'p TypedTrees,
    lexical_owner: SymbolHandle,
    target: SymbolHandle,
    receiver: Option<AppliedTraitScope<'p>>,
    application: TypeReferenceHandle,
) -> Result<
    Option<(
        SymbolHandle,
        typed_trees::trait_definition::AppliedTraitScope<'p>,
    )>,
    Vec<Diagnostic>,
> {
    if let Some(closed) = program.closed_requirement_application(target) {
        return closed
            .trait_scope
            .scope(program)
            .map(|scope| Some((closed.template, scope)))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "applied public call lost its retained source scope",
                )]
            });
    }
    // `Requirement<Arguments>::operation(...)` names its exact application;
    // an inherited operation resolves in that application's declaring scope.
    if application.is_valid() {
        let Some((owner, _)) = program.trait_machine_signature(target) else {
            return Ok(None);
        };
        return Ok(program
            .applied_requirement_scope(lexical_owner, application)
            .and_then(|scope| scope.declaring_scope(program, owner.symbol))
            .map(|scope| (target, scope)));
    }
    if program.trait_machine_signature(target).is_none()
        && let Some(machine) = program.machine_by_symbol(lexical_owner)
        && let Some(requirement) =
            validation::named_conformance_target_requirement(program, machine, target)
                .map_err(|message| vec![Diagnostic::error(message)])?
    {
        return Ok(Some((
            requirement.signature.symbol,
            requirement.application,
        )));
    }
    let template = program
        .closed_requirement_application(target)
        .map_or(target, |application| application.template);
    let Some((owner, _)) = program.trait_machine_signature(template) else {
        return Ok(None);
    };
    // Static provider slots inherit their exact application from the authored
    // requirement qualifier. A source routing carrier no longer supplies that
    // scope; selected maps and equal ABI signatures still cannot invent it.
    let qualifier_scope = receiver.and_then(|scope| scope.declaring_scope(program, owner.symbol));
    let scope = match qualifier_scope {
        Some(scope) => Some(scope),
        None if program.trait_type_parameters(owner).is_empty()
            && owner.lifetime_parameters.is_empty() =>
        {
            Some(typed_trees::trait_definition::AppliedTraitScope::abstract_scope(owner))
        }
        None => None,
    };
    Ok(scope.map(|scope| (template, scope)))
}

fn instance_in_scope(
    program: &TypedTrees,
    template: SymbolHandle,
    scope: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    invocation: &InvocationTuple,
) -> SymbolHandle {
    let arguments = &invocation.arguments;
    let Some((_, signature)) = program.trait_machine_signature(template) else {
        return SymbolHandle::invalid();
    };
    let parameters = program.state_signature_type_parameters(signature);
    if parameters.len() != arguments.len() {
        return SymbolHandle::invalid();
    }
    let mut matches = program
        .closed_requirement_applications()
        .filter(|application| {
            application.template == template
                && program
                    .requirement_application_matches_scope(application.signature.symbol, scope)
                && program
                    .expression_table
                    .static_machine_application_lifetimes(application.application)
                    == invocation.lifetimes
                && (invocation.lifetimes.is_empty()
                    || program
                        .expression_table
                        .static_machine_application(application.application)
                        .lexical_owner
                        == invocation.owner)
                && parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| {
                        let bindings = program
                            .expression_table
                            .static_machine_application_bindings(application.application);
                        let mut selected = bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol);
                        let Some(binding) = selected.next() else {
                            return false;
                        };
                        selected.next().is_none()
                            && program.static_binding_matches_argument(
                                &parameter.kind,
                                &binding.value,
                                argument,
                                invocation.owner,
                                program
                                    .expression_table
                                    .static_machine_application(application.application)
                                    .lexical_owner,
                            )
                    })
        });
    let Some(application) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        return SymbolHandle::invalid();
    }
    application.signature.symbol
}

/// Evidence packaging retains a separate expression for the same source
/// invocation. Its complete logical selection must be prepared and retargeted
/// alongside the ordinary statement; the binding's later statement position is
/// not a second invocation. Erased-only packages keep their own source point.
fn source_expression_roster(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    position: usize,
    statement: &typed_trees::statement::StatementNode,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut expressions = Vec::new();
    validation::collect_statement_expression_trees(program, statement, &mut expressions);
    for package in &program.proof_output_calls {
        if package.machine_symbol == machine
            && package.state_symbol == state
            && package
                .runtime_call_statement_index
                .unwrap_or(package.statement_index)
                == position
        {
            validation::expression_traversal::collect_expression_tree(
                program,
                package.call,
                &mut expressions,
            );
        }
    }
    expressions
}

/// Static calls instantiate their own complete invocation tuples. These rows
/// are public contract applications, never a finite dynamic-dispatch roster.
pub(super) fn invocation_tuples(
    program: &TypedTrees,
    requirement: SymbolHandle,
    scope: &typed_trees::trait_definition::AppliedTraitScope<'_>,
) -> Result<Vec<InvocationTuple>, Vec<Diagnostic>> {
    let mut tuples = Vec::new();
    let mut add = |owner,
                   target,
                   receiver,
                   application: TypeReferenceHandle,
                   dispatch: Option<&typed_trees::typed_trees::StaticRequirementDispatch>,
                   invocation: InvocationTuple|
     -> Result<(), Vec<Diagnostic>> {
        let selected = if let Some(dispatch) = dispatch {
            let applications = program.selected_conformance_applications(
                owner,
                dispatch.selection_application,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            if applications.len() != 1 {
                return Err(vec![Diagnostic::error(
                    "static method occurrence has no unique selected conformance",
                )]);
            }
            let template = program
                .closed_requirement_application(dispatch.requirement)
                .map_or(dispatch.requirement, |application| application.template);
            applications[0]
                .trait_scope
                .scope(program)
                .map(|scope| (template, scope))
        } else {
            source_scope(program, owner, target, receiver, application)?
        };
        let Some((template, selected)) = selected else {
            return Ok(());
        };
        if template == requirement
            && selected.same_application(program, scope)
            && !tuples.contains(&invocation)
        {
            tuples.push(invocation);
        }
        Ok(())
    };
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (position, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    add(
                        machine.symbol,
                        call.target_symbol,
                        validation::qualified_trait_scope(
                            program,
                            machine.symbol,
                            call.receiver_symbol,
                            validation::projected_statement_receiver_type_reference(
                                program, state, position, call,
                            ),
                        ),
                        TypeReferenceHandle::invalid(),
                        call.static_requirement_dispatch.as_ref(),
                        occurrence(program, machine.symbol, StaticArgumentCall::Statement(call)),
                    )?;
                }
                let expressions = source_expression_roster(
                    program,
                    machine.symbol,
                    state.symbol,
                    position,
                    statement,
                );
                for expression in expressions {
                    if let Some(call) = program.expression_table.call_node(expression) {
                        add(
                            machine.symbol,
                            call.target_symbol,
                            validation::qualified_trait_scope(
                                program,
                                machine.symbol,
                                program
                                    .expression_table
                                    .name_path_node(call.receiver)
                                    .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                                validation::expression_type_reference_in_state(
                                    program,
                                    state.symbol,
                                    position,
                                    call.receiver,
                                ),
                            ),
                            call.requirement_application,
                            call.static_requirement_dispatch.as_ref(),
                            occurrence(
                                program,
                                machine.symbol,
                                StaticArgumentCall::Expression(expression, call),
                            ),
                        )?;
                    }
                }
            }
        }
    }
    Ok(tuples)
}

/// The executable target remains the chosen provider. Only its public contract
/// coordinate is applied, through the same committed map and exact scope that
/// call admission checks. A provider's stronger private contract is irrelevant.
fn dispatch_instance(
    program: &TypedTrees,
    owner: SymbolHandle,
    dispatch: &typed_trees::typed_trees::StaticRequirementDispatch,
    invocation: &InvocationTuple,
) -> SymbolHandle {
    let applications = program.selected_conformance_applications(
        owner,
        dispatch.selection_application,
        dispatch.requirement,
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
    instance_in_scope(program, template, &scope, invocation)
}

pub(super) fn retarget(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut invocations = Vec::new();
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
                    invocations.push((
                        super::super::CallSite::Statement(
                            state.statement_nodes.handle_at(position as u32),
                        ),
                        machine.symbol,
                        call.target_symbol,
                    ));
                    if let Some(dispatch) = &call.static_requirement_dispatch {
                        let selected = dispatch_instance(
                            program,
                            machine.symbol,
                            dispatch,
                            &occurrence(
                                program,
                                machine.symbol,
                                StaticArgumentCall::Statement(call),
                            ),
                        );
                        if selected.is_valid() {
                            statement_dispatch_updates
                                .push((state.statement_nodes.handle_at(position as u32), selected));
                        }
                    }
                    let receiver = validation::qualified_trait_scope(
                        program,
                        machine.symbol,
                        call.receiver_symbol,
                        validation::projected_statement_receiver_type_reference(
                            program, state, position, call,
                        ),
                    );
                    let selected = instance(
                        program,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        TypeReferenceHandle::invalid(),
                        &occurrence(program, machine.symbol, StaticArgumentCall::Statement(call)),
                    )?;
                    if selected.is_valid() {
                        statement_updates
                            .push((state.statement_nodes.handle_at(position as u32), selected));
                    }
                }
                let expressions = source_expression_roster(
                    program,
                    machine.symbol,
                    state.symbol,
                    position,
                    statement,
                );
                for expression in expressions {
                    let Some(call) = program.expression_table.call_node(expression) else {
                        continue;
                    };
                    invocations.push((
                        super::super::CallSite::Expression(expression),
                        machine.symbol,
                        call.target_symbol,
                    ));
                    if let Some(dispatch) = &call.static_requirement_dispatch {
                        let selected = dispatch_instance(
                            program,
                            machine.symbol,
                            dispatch,
                            &occurrence(
                                program,
                                machine.symbol,
                                StaticArgumentCall::Expression(expression, call),
                            ),
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
                    let receiver = validation::qualified_trait_scope(
                        program,
                        machine.symbol,
                        program
                            .expression_table
                            .name_path_node(call.receiver)
                            .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                        validation::expression_type_reference_in_state(
                            program,
                            state.symbol,
                            position,
                            call.receiver,
                        ),
                    );
                    let selected = instance(
                        program,
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                        call.requirement_application,
                        &occurrence(
                            program,
                            machine.symbol,
                            StaticArgumentCall::Expression(expression, call),
                        ),
                    )?;
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
    for &(site, owner, target) in &invocations {
        let mut template = program
            .closed_requirement_application(target)
            .map_or(target, |closed| closed.template);
        if program.trait_machine_signature(template).is_none()
            && let Some((requirement, _)) =
                source_scope(program, owner, target, None, TypeReferenceHandle::invalid())?
        {
            template = requirement;
        }
        let Some((_, signature)) = program.trait_machine_signature(template) else {
            continue;
        };
        let parameters = program.state_signature_type_parameters(signature).to_vec();
        if parameters.is_empty() {
            continue;
        }
        let application = super::super::applications::retain_authored_call(program, site, owner);
        let arguments = program
            .expression_table
            .static_machine_application_arguments(application)
            .to_vec();
        if arguments.len() != parameters.len() {
            // Argument admission owns malformed source telescopes.
            continue;
        }
        let bindings = super::invocation_bindings(program, &parameters, &arguments)?;
        program
            .expression_table
            .static_machine_application_mut(application)
            .binding_declaration = template;
        program
            .expression_table
            .set_static_machine_application_bindings(application, &bindings);
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
    for (site, owner, _) in invocations {
        let super::super::CallSite::Expression(expression) = site else {
            continue;
        };
        let Some(call) = program.expression_table.call_node(expression).cloned() else {
            continue;
        };
        let selected_scope = if let Some(dispatch) = &call.static_requirement_dispatch {
            let applications = program.selected_conformance_applications(
                owner,
                dispatch.selection_application,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                continue;
            };
            application
                .trait_scope
                .scope(program)
                .map(|scope| (dispatch.requirement, scope))
        } else {
            source_scope(
                program,
                owner,
                call.target_symbol,
                None,
                call.requirement_application,
            )?
        };
        let Some((template, scope)) = selected_scope else {
            continue;
        };
        let Some((declaration, declared)) = program.trait_machine_signature(template) else {
            continue;
        };
        let Some(scope) = scope.declaring_scope(program, declaration.symbol) else {
            continue;
        };
        let Some(selected) = program
            .applied_callable_signature(&scope, template)
            .or_else(|| {
                (scope.substitutions.is_empty() && declaration.lifetime_parameters.is_empty())
                    .then_some(declared)
            })
            .cloned()
        else {
            continue;
        };
        let Some(bindings) = invocation_binding_views::public_bindings(program, owner, &call)?
        else {
            continue;
        };
        if let Some(signature) = program
            .prepare_requirement_invocation(owner, &call, &bindings, &selected)
            .map_err(|message| vec![Diagnostic::error(message)])?
        {
            let references =
                typed_trees::references::collect_signature_references(program, &signature);
            super::super::finalize_referenced_domain_instance_identities(program, references)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    Ok(())
}
