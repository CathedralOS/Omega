//! Public method inference consumes declared argument structure, not providers.
//! Retain the resulting tuple beside the authored operands before requirement
//! discovery needs it. Ordinary formation still admits every actual and contract.

use super::super::{CallSite, selection};
use crate::tables::{expressions_of, expressions_of_mut};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::{TypeParameter, TypeParameterKind},
    expression::StaticMachineArgument,
    machine::Machine,
    references::StaticArgumentCall,
    signature::StateSignature,
    state::State,
    types::TypeReferenceHandle,
};

struct Invocation {
    site: CallSite,
    owner: SymbolHandle,
    target: SymbolHandle,
    parameters: Vec<TypeParameter>,
    declaration_parameters: Vec<TypeParameter>,
    signature: StateSignature,
    selected: Vec<StaticMachineArgument>,
    operands: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
}

pub(super) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    loop {
        let mut pending = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (position, (statement_handle, statement)) in program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                    .enumerate()
                {
                    let expressions = super::calls::source_expression_roster(
                        program,
                        machine.symbol,
                        state.symbol,
                        position,
                        statement,
                    );
                    let sites = statement
                        .as_call()
                        .map(|_| CallSite::Statement(statement_handle))
                        .into_iter()
                        .chain(expressions.into_iter().map(CallSite::Expression));
                    for site in sites {
                        if seen.insert(site)
                            && let Some(invocation) = observe(program, machine, state, site)
                        {
                            pending.push(invocation);
                        }
                    }
                }
            }
        }
        let mut changed = false;
        for invocation in pending {
            let Some(arguments) = infer(program, &invocation)? else {
                continue;
            };
            // Inference reads the applied signature's fresh local symbols.
            // Source retention is keyed by the original declaration telescope;
            // the ordinary invocation owner rejoins it to the applied view.
            let bindings = super::invocation_bindings(
                program,
                &invocation.declaration_parameters,
                &arguments,
            )?;
            let application = super::super::applications::retain_authored_call(
                program,
                invocation.site,
                invocation.owner,
            );
            let source = expressions_of_mut(program).static_machine_application_mut(application);
            if source.lexical_owner != invocation.owner
                || source.registration_operation != invocation.target
                || (source.binding_declaration.is_valid()
                    && source.binding_declaration != invocation.target)
            {
                return Err(vec![Diagnostic::error(
                    "public method inference lost its exact source application",
                )]);
            }
            source.binding_declaration = invocation.target;
            expressions_of_mut(program)
                .set_static_machine_application_bindings(application, &bindings);
            match invocation.site {
                CallSite::Expression(expression) => {
                    let call = expressions_of(program)
                        .call_node(expression)
                        .expect("observed public expression call")
                        .clone();
                    program.prepare_requirement_invocation(
                        invocation.owner,
                        &call,
                        &bindings,
                        &invocation.signature,
                    )
                }
                CallSite::Statement(statement) => {
                    let call = program
                        .statement_table
                        .statement(statement)
                        .as_call()
                        .expect("observed public statement call")
                        .clone();
                    program.prepare_requirement_statement_invocation(
                        invocation.owner,
                        &call,
                        &bindings,
                        &invocation.signature,
                    )
                }
            }
            .map_err(|message| vec![Diagnostic::error(message)])?;
            changed = true;
        }
        if !changed {
            return Ok(());
        }
        // An inferred inner call may expose the exact type of an outer actual.
        // Refresh the ordinary logical signatures before observing those uses.
        program
            .prepare_applied_callable_signatures()
            .map_err(|message| {
                vec![Diagnostic::error(format!(
                    "inferred public method signature: {message}"
                ))]
            })?;
    }
}

fn observe(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    site: CallSite,
) -> Option<Invocation> {
    let (call, signature, arguments) = match site {
        CallSite::Expression(expression) => {
            let call = expressions_of(program).call_node(expression)?;
            if call.static_requirement_dispatch.is_some()
                || program
                    .closed_requirement_application(call.target_symbol)
                    .is_some()
            {
                return None;
            }
            (
                StaticArgumentCall::Expression(expression, call),
                program.requirement_call_signature(machine.symbol, call)?,
                expressions_of(program).expression_handles(call.arguments),
            )
        }
        CallSite::Statement(statement) => {
            let call = program.statement_table.statement(statement).as_call()?;
            if call.static_requirement_dispatch.is_some()
                || program
                    .closed_requirement_application(call.target_symbol)
                    .is_some()
            {
                return None;
            }
            (
                StaticArgumentCall::Statement(call),
                program.requirement_statement_call_signature(machine.symbol, call)?,
                program.statement_table.expression_handles(call.arguments),
            )
        }
    };
    let target = call.registration_operation(program);
    let (_, declaration) = program.trait_machine_signature(target)?;
    let parameters = program.state_signature_type_parameters(signature);
    let declaration_parameters = program.state_signature_type_parameters(declaration);
    if parameters.len() != declaration_parameters.len()
        || declaration_parameters
            .iter()
            .zip(parameters)
            .any(|(declared, applied)| {
                program.applied_signature_parameter(declaration, signature, declared.symbol)
                    != Some(applied.symbol)
                    || std::mem::discriminant(&declared.kind)
                        != std::mem::discriminant(&applied.kind)
            })
    {
        return None;
    }
    let selected = call.arguments(program);
    if selected.len() >= parameters.len() {
        return None;
    }
    if call.application().is_valid() {
        let source = expressions_of(program).static_machine_application(call.application());
        let bindings =
            expressions_of(program).static_machine_application_bindings(call.application());
        if source.lexical_owner == machine.symbol
            && source.binding_declaration == target
            && bindings.len() == declaration_parameters.len()
            && declaration_parameters.iter().all(|parameter| {
                bindings
                    .iter()
                    .filter(|binding| binding.parameter == parameter.symbol)
                    .count()
                    == 1
            })
        {
            return None;
        }
    }
    let runtime = program.state_signature_parameters(signature);
    if runtime.len() != arguments.len() {
        return None;
    }
    let operands = runtime
        .iter()
        .zip(arguments)
        .filter_map(|(required, argument)| {
            let actual =
                validation::expression_result_type_reference(program, machine, state, *argument)?;
            if !program.callable_body_has_open_parameters(machine)
                && selection::names_type_binder(program, actual)
            {
                return None;
            }
            Some((required.type_reference, actual))
        })
        .collect();
    Some(Invocation {
        site,
        owner: machine.symbol,
        target,
        parameters: parameters.to_vec(),
        declaration_parameters: declaration_parameters.to_vec(),
        signature: signature.clone(),
        selected: selected.to_vec(),
        operands,
    })
}

fn infer(
    program: &TypedTrees,
    invocation: &Invocation,
) -> Result<Option<Vec<StaticMachineArgument>>, Vec<Diagnostic>> {
    let types = invocation
        .parameters
        .iter()
        .filter_map(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                .then(|| (parameter.symbol, parameter.name.as_str().to_owned()))
        })
        .collect::<Vec<_>>();
    let constants = invocation
        .parameters
        .iter()
        .filter_map(|parameter| {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return None;
            };
            Some((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                type_reference,
            ))
        })
        .collect::<Vec<_>>();
    let context = selection::BindingContext::new(program, &types, &constants, None, 0);
    let mut proposals = selection::StaticProposals::default();
    for (required, actual) in &invocation.operands {
        selection::infer_static_bindings(context, *required, *actual, &mut proposals);
    }
    let mut inferred = vec![None; invocation.parameters.len()];
    for (is_const, symbol, actual) in proposals
        .types
        .into_iter()
        .map(|(_, ordinal, actual)| (false, types[ordinal].0, actual))
        .chain(
            proposals
                .constants
                .into_iter()
                .map(|(_, ordinal, actual)| (true, constants[ordinal].0, actual)),
        )
    {
        let ordinal = invocation
            .parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .expect("proposal belongs to this public telescope");
        // Explicit source selections are fixed. Compatibility with those
        // operands is an ordinary call obligation after substitution.
        if ordinal < invocation.selected.len() {
            continue;
        }
        if let Some(previous) = inferred[ordinal] {
            let agrees = if is_const {
                selection::same_const_identity(program, previous, actual)
            } else {
                program.normalized_type_identity(previous)
                    == program.normalized_type_identity(actual)
            };
            if !agrees {
                return Err(vec![Diagnostic::error(format!(
                    "public method inference has conflicting structural operands for `{}`",
                    invocation.parameters[ordinal].name,
                ))]);
            }
        } else {
            inferred[ordinal] = Some(actual);
        }
    }
    let mut complete = invocation.selected.clone();
    for actual in inferred.into_iter().skip(complete.len()) {
        let Some(actual) = actual else {
            return Ok(None);
        };
        complete.push(
            program
                .static_operand_from_type_reference(actual)
                .map_err(|message| vec![Diagnostic::error(message)])?,
        );
    }
    Ok(Some(complete))
}
