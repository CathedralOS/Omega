//! Exact closed selection identity, independent of proof-output or execution rungs.
use symbols::SymbolHandle;
use typed_trees::typed_trees::StaticRequirementDispatch;

/// A rewritten private target is not selection authority. Join the owner's
/// full application commitment to one public requirement and one private row.
/// Callable admission, contracts and operand custody remain with the caller.
pub fn closed_requirement_dispatch_is_valid(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    target: SymbolHandle,
    dispatch: &StaticRequirementDispatch,
) -> bool {
    if dispatch.application_commitment.is_zero() || dispatch.application_report_fingerprint == 0 {
        return false;
    }
    let applications = program.selected_conformance_applications(
        owner,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return false;
    };
    let mut executable = match program.machine_holding_state(target) {
        Some(machine) => machine.symbol,
        None => return false,
    };
    let supplying_application = if dispatch.selection_application.is_valid() {
        dispatch.selection_application
    } else {
        let supplying = program
            .machine_specializations
            .iter()
            .filter(|row| row.instance == owner)
            .collect::<Vec<_>>();
        let [supplying] = supplying.as_slice() else {
            return false;
        };
        supplying.binding_application
    };
    let supplying_owner = program
        .expression_table
        .static_machine_application(supplying_application)
        .lexical_owner;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&executable) {
            return false;
        }
        visited.push(executable);
        for (_, capture) in program
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| capture.instance == executable)
        {
            if capture_binding_is_exact(program, capture).is_err()
                || program
                    .expression_table
                    .static_machine_application(capture.supplying_application)
                    .lexical_owner
                    != supplying_owner
            {
                return false;
            }
        }
        match program.executable_application_template(executable) {
            Ok(Some(template)) => executable = template,
            Ok(None) => break,
            Err(_) => return false,
        }
    }
    if program.realization_entry_template(target, application)
        != Some((dispatch.realization_machine, dispatch.realization_state))
    {
        return false;
    }
    if dispatch.contract_signature.is_valid() {
        let Some(closed) = program.closed_requirement_application(dispatch.contract_signature)
        else {
            return false;
        };
        let Some(scope) = application.trait_scope.scope(program) else {
            return false;
        };
        if closed.declaring_trait != dispatch.declaring_trait
            || closed.template != dispatch.requirement
            || !program.requirement_application_matches_scope(dispatch.contract_signature, &scope)
        {
            return false;
        }
    }
    let requirement = dispatch.requirement;
    let Some(application_trait) = program.trait_definition_by_symbol(application.trait_definition)
    else {
        return false;
    };
    let mut pending = vec![application_trait];
    let mut visited = Vec::new();
    let mut declares = false;
    while let Some(definition) = pending.pop() {
        if visited.contains(&definition.symbol) {
            continue;
        }
        visited.push(definition.symbol);
        if definition.symbol == dispatch.declaring_trait {
            declares = true;
            break;
        }
        for requirement in program.trait_requirements(definition) {
            if let Some(parent) = program.trait_definition_by_symbol(requirement.symbol) {
                pending.push(parent);
            }
        }
    }
    if !declares {
        return false;
    }
    let Some(declaration) = program.trait_definition_by_symbol(dispatch.declaring_trait) else {
        return false;
    };
    if program
        .trait_machine_signatures(declaration)
        .iter()
        .filter(|signature| signature.symbol == requirement)
        .count()
        != 1
    {
        return false;
    }
    let Some(selected) = program.conformance_by_symbol(application.declaration) else {
        return false;
    };
    // A requirement owns one selection, not merely one matching tuple among
    // conflicting alternatives. Check cardinality before the private identity.
    let selected_rows = selected
        .rows
        .as_slice()
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait && row.requirement == requirement
        })
        .collect::<Vec<_>>();
    let application_rows = application
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait && row.requirement == requirement
        })
        .collect::<Vec<_>>();
    let ([selected_row], [application_row]) =
        (selected_rows.as_slice(), application_rows.as_slice())
    else {
        return false;
    };
    selected_row.realization_machine == dispatch.realization_machine
        && selected_row.realization_state == dispatch.realization_state
        && application_row.realization_machine == dispatch.realization_machine
        && application_row.realization_state == dispatch.realization_state
}

/// Replay a capture substitution from the exact original selected evidence
/// operand. Generated executable rows cannot attest their own capture tuple.
pub(crate) fn capture_binding_is_exact(
    program: &dyn crate::CheckingInput,
    capture: &typed_trees::typed_trees::CapturedMachineApplication,
) -> Result<(), &'static str> {
    conformance_binding_is_exact(
        program,
        capture.supplying_application,
        capture.binding_application,
        &capture.conformance_application,
    )?;
    let application = &capture.conformance_application;
    let declaration = program
        .conformance_by_symbol(application.declaration)
        .ok_or("capture lost its original declaration")?;
    let count = program
        .conformance_type_parameters(declaration)
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Machine { .. }
            )
        })
        .count();
    let selected = program
        .expression_table
        .static_machine_application_selected_machines(capture.binding_application);
    if selected.len() != count {
        return Err("capture executable callback roster differs from map telescope");
    }
    if count != 0 {
        if !application.callable_application.is_valid()
            || selected
                != program
                    .expression_table
                    .static_machine_application_selected_machines(application.callable_application)
        {
            return Err(
                "capture executable callbacks differ from their exact admitted map occurrence",
            );
        }
    }
    Ok(())
}

pub(crate) fn conformance_binding_is_exact(
    program: &dyn crate::CheckingInput,
    supplying_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    binding_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
) -> Result<(), &'static str> {
    use typed_trees::static_applications::StaticMachineBindingValue;
    if !binding_application.is_valid() || !supplying_application.is_valid() {
        return Err("capture has missing supplying or binding application");
    }
    let source = program
        .expression_table
        .static_machine_application(supplying_application);
    let binding = program
        .expression_table
        .static_machine_application(binding_application);
    if binding.lexical_owner != source.lexical_owner
        || binding.registration_operation != application.declaration
        || binding.binding_declaration != application.declaration
    {
        return Err("capture binding owner or map declaration differs from supplying application");
    }
    let selected = if let Some(selection) = &source.conformance_selection {
        program
            .expression_table
            .static_machine_application_arguments(supplying_application)
            .get(selection.operand_ordinal as usize)
            .and_then(|argument| {
                crate::conformance_applications::close_conformance_application(program, argument)
                    .ok()
            })
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        program
            .expression_table
            .static_machine_application_bindings(supplying_application)
            .iter()
            .filter_map(|binding| match &binding.value {
                StaticMachineBindingValue::Conformance(argument)
                    if argument.symbol == application.declaration =>
                {
                    crate::conformance_applications::close_conformance_application(
                        program, argument,
                    )
                    .ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut expected = application.clone();
    expected.callable_application = Default::default();
    expected.trait_scope = Default::default();
    let selected = selected
        .into_iter()
        .filter(|selection| selection == &expected)
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err("capture supplying application has no unique exact selected conformance");
    }
    let Some(declaration) = program.conformance_by_symbol(application.declaration) else {
        return Err("capture original conformance declaration is absent");
    };
    let parameters = program.conformance_type_parameters(declaration);
    let bindings = program
        .expression_table
        .static_machine_application_bindings(binding_application);
    if parameters.len() != application.arguments.len() || bindings.len() != parameters.len() {
        return Err("capture tuple or binding roster differs from map telescope");
    }
    for (parameter, argument) in parameters.iter().zip(&application.arguments) {
        let selected = bindings
            .iter()
            .filter(|binding| binding.parameter == parameter.symbol)
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            return Err("capture binder has no unique selected operand");
        };
        let matches = program.static_binding_matches_argument(
            &parameter.kind,
            &selected.value,
            argument,
            source.lexical_owner,
            binding.lexical_owner,
        );
        if !matches {
            return Err("capture operand does not match its exact original selected argument");
        }
    }
    let lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(binding_application);
    let matches = lifetimes.len() == declaration.lifetime_parameters.len()
        && lifetimes.len() == application.lifetime_arguments.len()
        && lifetimes
            .iter()
            .zip(
                declaration
                    .lifetime_parameters
                    .iter()
                    .zip(&application.lifetime_arguments),
            )
            .all(|(binding, (parameter, argument))| {
                binding.parameter == *parameter && binding.argument.as_str() == argument
            });
    if matches {
        Ok(())
    } else {
        Err("capture lifetime tuple differs from original map telescope")
    }
}
