//! A selected call has one source invocation but two declaration namespaces:
//! its public member and its private executable. Prepare a transient public
//! binding view without overwriting the executable's retained substitution.
//! Inferred operands come only from this invocation's exact selected realizer.

use crate::tables::expressions_of;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::StaticMachineArgument,
    references::StaticArgumentCall,
    static_applications::{StaticMachineBinding, StaticMachineBindingValue},
};

use super::super::selected_applications;

/// Derive a public demand from an already selected nominal call. This does
/// not admit satisfaction or replace the source invocation: immutable checking
/// still compares its complete nominal operands and regions.
pub(super) fn nominal_source<'program>(
    program: &'program TypedTrees,
    owner: SymbolHandle,
    binder: SymbolHandle,
    target: SymbolHandle,
) -> Result<
    Option<(
        SymbolHandle,
        typed_trees::trait_definition::AppliedTraitScope<'program>,
    )>,
    Vec<Diagnostic>,
> {
    let Some(typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition,
        requirement,
    }) = program.retained_static_machine_contract(binder)
    else {
        return Ok(None);
    };
    let Some(provider) = program.machine_holding_state(target) else {
        return Ok(None);
    };
    let authored = program.lifetime_declaration_owner(provider.symbol);
    let Some(entry) = program
        .machine_by_symbol(authored)
        .and_then(|machine| program.machine_states(machine).first())
    else {
        return Ok(None);
    };
    if !program
        .static_machine_binding_selects_target(owner, binder, entry.symbol)
        .map_err(|reason| vec![Diagnostic::error(reason)])?
        || !program.instantiated_machine_state_matches(
            provider.symbol,
            authored,
            entry.symbol,
            target,
        )
    {
        return Ok(None);
    }
    let template = program
        .closed_requirement_application(requirement.symbol)
        .map_or(requirement.symbol, |closed| closed.template);
    program
        .machine_conformance_requirement_scope(provider.symbol, trait_definition.symbol, template)
        .map(|scope| scope.map(|scope| (template, scope)))
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

pub(super) fn public_bindings(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    call: StaticArgumentCall<'_>,
) -> Result<Option<Vec<StaticMachineBinding>>, Vec<Diagnostic>> {
    let invocation = call.application();
    let dispatch = match call {
        StaticArgumentCall::Expression(_, call) => call.static_requirement_dispatch.as_ref(),
        StaticArgumentCall::Statement(call) => call.static_requirement_dispatch.as_ref(),
    };
    let Some(dispatch) = dispatch else {
        return Ok(Some(
            expressions_of(program)
                .static_machine_application_bindings(invocation)
                .to_vec(),
        ));
    };
    if program
        .closed_requirement_application(dispatch.contract_symbol())
        .is_some()
    {
        return Ok(Some(Vec::new()));
    }
    let Some((_, signature)) = program.trait_machine_signature(dispatch.requirement) else {
        return Ok(None);
    };
    let parameters = program.state_signature_type_parameters(signature).to_vec();
    let Some(mut arguments) = complete_arguments(program, owner, call) else {
        return Ok(None);
    };
    // This public binding view is derivation data. Normalization must not
    // consume the source recipe retained on the invocation application.
    super::super::super::body_rewriting::normalize_const_arguments(program, owner, &mut arguments)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    super::super::invocation_bindings(program, owner, &parameters, &arguments).map(Some)
}

/// Recover the method's complete tuple from its own retained inference.
/// Captures never fill missing invocation operands. The private binder names
/// are transported by declaration ordinal only after rejoining the exact map
/// and executable row; their contracts do not enter the public signature.
pub(super) fn complete_arguments(
    program: &TypedTrees,
    owner: SymbolHandle,
    call: StaticArgumentCall<'_>,
) -> Option<Vec<StaticMachineArgument>> {
    let (target, dispatch) = match call {
        StaticArgumentCall::Expression(_, call) => (
            call.target_symbol,
            call.static_requirement_dispatch.as_ref(),
        ),
        StaticArgumentCall::Statement(call) => (
            call.target_symbol,
            call.static_requirement_dispatch.as_ref(),
        ),
    };
    let requirement = dispatch.map_or_else(
        || call.registration_operation(program),
        |dispatch| dispatch.requirement,
    );
    let (_, signature) = program.trait_machine_signature(requirement)?;
    let parameters = program.state_signature_type_parameters(signature);
    let arguments = call.arguments(program);
    if arguments.len() == parameters.len() {
        return Some(arguments.to_vec());
    }
    if arguments.len() > parameters.len() || !call.application().is_valid() {
        return None;
    }
    let invocation = call.application();
    let source = expressions_of(program).static_machine_application(invocation);
    if source.lexical_owner != owner {
        return None;
    }
    let bindings = expressions_of(program).static_machine_application_bindings(invocation);
    if dispatch.is_none() {
        // Direct public calls have no private realizer telescope. Structural
        // inference retains their own exact public bindings on this occurrence.
        if source.binding_declaration != requirement || bindings.len() != parameters.len() {
            return None;
        }
        return parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| {
                let mut selected = bindings
                    .iter()
                    .filter(|binding| binding.parameter == parameter.symbol);
                let binding = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                    | StaticMachineBindingValue::Const(reference) => {
                        program.static_operand_from_type_reference(*reference).ok()
                    }
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => Some(argument.clone()),
                    StaticMachineBindingValue::RuntimeValue {
                        subject,
                        expression,
                        ..
                    } => runtime_argument(program, call, ordinal, *subject, *expression),
                }
            })
            .collect();
    }
    let dispatch = dispatch?;
    let applications = selected_applications(program, owner, dispatch);
    let [application] = applications.as_slice() else {
        return None;
    };
    if program.realization_entry_template(target, application)?
        != (dispatch.realization_machine, dispatch.realization_state)
    {
        return None;
    }
    let declaration = source.binding_declaration;
    let machine = program.machine_by_symbol(declaration)?;
    let private_parameters = program
        .requirement_callable_parameters(signature, &program.machine_callable_parameters(machine));
    if private_parameters.len() != parameters.len() || bindings.len() != parameters.len() {
        return None;
    }
    let mut original = declaration;
    let mut visited = Vec::new();
    while original != dispatch.realization_machine {
        if visited.contains(&original) {
            return None;
        }
        visited.push(original);
        let template = program.executable_application_template(original).ok()??;
        original = template;
    }
    let mut complete = Vec::with_capacity(parameters.len());
    for (ordinal, private) in private_parameters.into_iter().enumerate() {
        let mut selected = bindings
            .iter()
            .filter(|binding| binding.parameter == private.symbol);
        let binding = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        complete.push(match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => program
                .static_operand_from_type_reference(*reference)
                .ok()?,
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
            StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } => runtime_argument(program, call, ordinal, *subject, *expression)?,
        });
    }
    Some(complete)
}

/// Restore a runtime subject from this occurrence's source binding. Executable
/// machine selections cannot supply logical operands. An expression wrapper is
/// usable only when it names the exact retained Value evaluation in this slot.
fn runtime_argument(
    program: &TypedTrees,
    call: StaticArgumentCall<'_>,
    ordinal: usize,
    subject: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<StaticMachineArgument> {
    if subject.is_valid() && !expression.is_valid() {
        return Some(StaticMachineArgument {
            symbol: subject,
            path: Box::from([program.symbols.name(subject).into()]),
            ..Default::default()
        });
    }
    if !expression.is_valid() {
        return None;
    }
    let live = match call {
        StaticArgumentCall::Expression(_, call) => call.machine_arguments.get(ordinal),
        StaticArgumentCall::Statement(call) => call.machine_arguments.get(ordinal),
    }?;
    (super::super::super::const_arguments::runtime_subject_expression(program, live)
        == Some(expression))
    .then(|| live.clone())
}
