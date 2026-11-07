//! Prepare the registrar named by an installation request in its own scope.
//! The request is a declaration application, not a synthetic runtime call.
//! Immutable validation still owns its callback and installation obligations.

use super::super::{CallSite, Diagnostic, SymbolHandle, TypedTrees};
use crate::tables::{expressions_of, expressions_of_mut};
use typed_trees::expression::{InstalledInterruptOperationRequest, StaticMachineArgument};
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBindingValue, StaticMachineLifetimeBinding,
};
use typed_trees::trait_definition::{AppliedTraitScope, RetainedTraitScope};

/// Run after ordinary calls have settled. A newly selected provider body goes
/// through another ordinary specialization round before this pass completes.
pub(crate) fn prepare(program: &mut TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    let before = (
        program.machines().len(),
        program.closed_requirement_applications().count(),
    );
    let mut sites = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call()
                    && let Some(request) = &call.installed_interrupt_operation
                {
                    sites.push((CallSite::Statement(handle), machine.symbol, request.clone()));
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for handle in expressions {
                    if let Some(call) = expressions_of(program).call_node(handle)
                        && let Some(request) = &call.installed_interrupt_operation
                        && !sites
                            .iter()
                            .any(|(site, _, _)| *site == CallSite::Expression(handle))
                    {
                        sites.push((
                            CallSite::Expression(handle),
                            machine.symbol,
                            request.clone(),
                        ));
                    }
                }
            }
        }
    }
    for (site, owner, mut request) in sites {
        if !request.registrar_application.is_valid() {
            let (application, target, receiving_scope) =
                prepare_registrar(program, owner, &request)?;
            request.registrar_application = application;
            request.registrar_target = target;
            request.registrar_receiving_scope = receiving_scope;
        }
        match site {
            CallSite::Expression(handle) => {
                let typed_trees::expression::ExpressionNode::Call(call) =
                    expressions_of_mut(program).expression_mut(handle)
                else {
                    unreachable!("retained installation expression remains a call")
                };
                call.installed_interrupt_operation = Some(request);
            }
            CallSite::Statement(handle) => {
                let typed_trees::statement::StatementNode::Call(call) =
                    program.statement_table.statement_mut(handle)
                else {
                    unreachable!("retained installation statement remains a call")
                };
                call.installed_interrupt_operation = Some(request);
            }
        }
    }
    Ok(before
        != (
            program.machines().len(),
            program.closed_requirement_applications().count(),
        ))
}

fn prepare_registrar(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    request: &InstalledInterruptOperationRequest,
) -> Result<
    (
        StaticMachineApplicationHandle,
        SymbolHandle,
        RetainedTraitScope,
    ),
    Vec<Diagnostic>,
> {
    // Normalization owns a copy. It must not rewrite the source recipe that
    // immutable argument checking and future specialized copies still consume.
    let authored = expressions_of_mut(program).copy_static_argument_from_self(&request.registrar);
    let mut operand = expressions_of_mut(program).copy_static_argument_from_self(&authored);
    super::super::normalize_const_arguments(program, owner, std::slice::from_mut(&mut operand))
        .map_err(|reason| failure(&reason))?;
    let authored_member = leaf(program, &authored)?;
    let member = leaf(program, &operand)?;
    let requirement = member.symbol;
    let (declaring_trait, declared) = program
        .trait_machine_signature(requirement)
        .map(|(definition, signature)| (definition.symbol, signature.clone()))
        .ok_or_else(|| failure("installation registrar lost its exact requirement declaration"))?;
    let receiving_environment = if operand
        .application
        .as_ref()
        .is_some_and(|app| app.member.is_valid())
    {
        let mut qualifier = operand.clone();
        qualifier
            .application
            .as_mut()
            .expect("qualified operand")
            .member = Default::default();
        let reference = program
            .static_operand_type_reference_in_scope(&qualifier, owner)
            .map_err(failure)?;
        let scope = AppliedTraitScope::from_type_reference(program, reference, owner)
            .filter(|scope| scope.declaration.symbol == request.registrar_trait)
            .ok_or_else(|| {
                failure("installation registrar has no complete selected owner application")
            })?;
        scope.environment()
    } else {
        let selected = program
            .trait_definition_by_symbol(request.registrar_trait)
            .ok_or_else(|| failure("installation registrar lost its selected trait"))?;
        AppliedTraitScope::abstract_scope(selected).environment()
    };
    let environment = receiving_environment
        .scope(program)
        .ok_or_else(|| failure("installation registrar lost its selected application scope"))?
        .declaring_scope(program, declaring_trait)
        .ok_or_else(|| failure("installation registrar has no unique declaring application"))?
        .environment();
    let retained_receiving_scope = receiving_environment.retain(program).map_err(failure)?;
    let parameters = program.state_signature_type_parameters(&declared).to_vec();
    let source_arguments = authored_member
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    let arguments = member
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    let lifetimes = member.application.as_ref().map_or(&[][..], |application| {
        application.lifetime_arguments.as_ref()
    });
    let application = expressions_of_mut(program).retain_static_machine_application(
        requirement,
        declared.name.clone(),
        source_arguments,
    );
    let source = expressions_of_mut(program).static_machine_application_mut(application);
    source.lexical_owner = owner;
    source.binding_declaration = requirement;
    // An incomplete or still symbolic source tuple remains available for the
    // ordinary checker. It is never a closed registrar or a native permission.
    if parameters.len() != arguments.len() || declared.lifetime_parameters.len() != lifetimes.len()
    {
        return Ok((
            application,
            SymbolHandle::invalid(),
            retained_receiving_scope,
        ));
    }
    let bindings = super::super::requirement_applications::invocation_bindings(
        program,
        owner,
        &parameters,
        arguments,
    )?;
    expressions_of_mut(program).set_static_machine_application_bindings(application, &bindings);
    let lifetime_bindings = declared
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(lifetimes.iter().cloned())
        .map(|(parameter, argument)| StaticMachineLifetimeBinding {
            parameter,
            argument,
        })
        .collect::<Vec<_>>();
    expressions_of_mut(program).set_static_machine_application_lifetimes(
        application,
        requirement,
        &lifetime_bindings,
    );
    if super::static_operands::argument_is_open(program, &operand) {
        return Ok((
            application,
            SymbolHandle::invalid(),
            retained_receiving_scope,
        ));
    }
    // An inherited member may not use every selected child's operand. Close
    // those operands too before preparing the declaring parent's contract.
    let receiving_scope = receiving_environment
        .scope(program)
        .ok_or_else(|| failure("installation registrar lost its selected application scope"))?;
    program
        .require_receiving_trait_operands(&receiving_scope)
        .map_err(failure)?;
    let scope = environment
        .scope(program)
        .ok_or_else(|| failure("installation registrar lost its owning application scope"))?;
    program
        .require_receiving_trait_operands(&scope)
        .map_err(failure)?;
    let trait_arguments = program
        .materialize_trait_application_arguments(&environment)
        .map_err(failure)?;
    let target = program
        .instantiate_requirement_application(
            &environment,
            requirement,
            &trait_arguments,
            application,
        )
        .map_err(failure)?;
    let signature = program
        .closed_requirement_application(target)
        .ok_or_else(|| failure("installation registrar lost its closed requirement"))?
        .signature
        .clone();
    let demands = super::invocations::machine_demands(
        program,
        program.requirement_invocation_contract_parameters(&signature),
    );
    let operands = bindings
        .iter()
        .filter_map(|binding| match binding.value {
            StaticMachineBindingValue::Type(actual) | StaticMachineBindingValue::Const(actual) => {
                Some((binding.parameter, actual))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut selected = bindings
        .iter()
        .filter_map(|binding| match &binding.value {
            StaticMachineBindingValue::Machine(argument) => Some(argument.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if selected.len() != demands.len() {
        return Err(failure(
            "installation registrar changed its complete machine binder roster",
        ));
    }
    for (ordinal, (argument, demand)) in selected.iter_mut().zip(&demands).enumerate() {
        if let Some(entry) = super::providers::close(
            program,
            argument,
            demand.parameter,
            &demand.signature,
            &operands,
            application,
            ordinal,
        )? {
            argument.symbol = entry;
            argument.application = None;
        }
    }
    expressions_of_mut(program)
        .set_static_machine_application_selected_machines(application, &selected);
    program
        .prepare_selected_machine_contracts(application)
        .map_err(failure)?;
    Ok((application, target, retained_receiving_scope))
}

fn leaf(
    program: &TypedTrees,
    root: &StaticMachineArgument,
) -> Result<StaticMachineArgument, Vec<Diagnostic>> {
    let mut operand = root;
    let mut selected_member = false;
    while let Some(application) = &operand.application
        && application.member.is_valid()
    {
        // The prepared scope below owns one trait application and its exact
        // requirement member. Another applied owner needs its own scope;
        // selecting only its final member would discard those operands.
        if selected_member {
            return Err(failure(
                "installation registrar has an additional applied owner; its complete qualified scope is not prepared",
            ));
        }
        operand = expressions_of(program)
            .static_member(application.member)
            .ok_or_else(|| {
                failure("installation registrar member lost its expression-table owner")
            })?;
        selected_member = true;
    }
    Ok(operand.clone())
}

fn failure(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}
