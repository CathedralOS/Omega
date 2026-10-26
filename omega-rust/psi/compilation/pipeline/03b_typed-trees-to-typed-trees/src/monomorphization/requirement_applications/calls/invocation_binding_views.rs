//! A selected call has one source invocation but two declaration namespaces:
//! its public member and its private executable. Prepare a transient public
//! binding view without overwriting the executable's retained substitution.
//! Inferred operands come only from this invocation's exact selected realizer.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{StaticMachineArgument, TableCallExpression},
    references::StaticArgumentCall,
    static_applications::{StaticMachineBinding, StaticMachineBindingValue},
};

pub(super) fn public_bindings(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    call: &TableCallExpression,
) -> Result<Option<Vec<StaticMachineBinding>>, Vec<Diagnostic>> {
    let invocation = call.authored_machine_application;
    let Some(dispatch) = &call.static_requirement_dispatch else {
        return Ok(Some(
            program
                .expression_table
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
    let Some(mut arguments) = complete_arguments(
        program,
        owner,
        StaticArgumentCall::Expression(Default::default(), call),
    ) else {
        return Ok(None);
    };
    // This public binding view is derivation data. Normalization must not
    // consume the source recipe retained on the invocation application.
    super::super::super::body_rewriting::normalize_const_arguments(program, owner, &mut arguments)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    super::super::invocation_bindings(program, &parameters, &arguments).map(Some)
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
    let dispatch = dispatch?;
    let (_, signature) = program.trait_machine_signature(dispatch.requirement)?;
    let parameters = program.state_signature_type_parameters(signature);
    let arguments = call.arguments(program);
    if arguments.len() == parameters.len() {
        return Some(arguments.to_vec());
    }
    if arguments.len() > parameters.len() || !call.application().is_valid() {
        return None;
    }
    let invocation = call.application();
    let source = program
        .expression_table
        .static_machine_application(invocation);
    if source.lexical_owner != owner {
        return None;
    }
    let bindings = program
        .expression_table
        .static_machine_application_bindings(invocation);
    let applications = program.selected_conformance_applications(
        owner,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
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
    let private_parameters = program.machine_type_parameters(machine);
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
    for private in private_parameters {
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
            // A runtime value subject is not a static invocation operand.
            StaticMachineBindingValue::RuntimeValue { .. } => return None,
        });
    }
    Some(complete)
}
