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
    let reject = |_reason: &str| false;
    if dispatch.application_commitment.is_zero() || dispatch.application_report_fingerprint == 0 {
        return reject("missing-application-commitment");
    }
    let applications = program.selected_conformance_applications(
        owner,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return reject("application-not-unique");
    };
    let mut executable = match program.machine_holding_state(target) {
        Some(machine) => machine.symbol,
        None => return reject("missing-executable-target-owner"),
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
            return reject("supplying-specialization-not-unique");
        };
        supplying.binding_application
    };
    let supplying_owner = program
        .expression_table
        .static_machine_application(supplying_application)
        .lexical_owner;
    // The public call may be checked before an open map can acquire an
    // executable clone. Its retained telescope still owes the same exact
    // supplying-application join as a materialized capture.
    if application.callable_application.is_valid()
        && conformance_binding_is_exact(
            program,
            supplying_application,
            application.callable_application,
            application,
        )
        .is_err()
    {
        return reject("supplying-map-telescope");
    }
    let mut visited = Vec::new();
    loop {
        if visited.contains(&executable) {
            return reject("executable-template-cycle");
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
                return reject("capture-binding-or-supplying-owner");
            }
        }
        match program.executable_application_template(executable) {
            Ok(Some(template)) => executable = template,
            Ok(None) => break,
            Err(_) => {
                return reject("executable-template-error");
            }
        }
    }
    if program.realization_entry_template(target, application)
        != Some((dispatch.realization_machine, dispatch.realization_state))
    {
        return reject("realization-entry-template");
    }
    if dispatch.contract_signature.is_valid() {
        let Some(closed) = program.closed_requirement_application(dispatch.contract_signature)
        else {
            return reject("missing-closed-public-contract");
        };
        let Some(scope) = application.trait_scope.scope(program) else {
            return reject("missing-public-trait-scope");
        };
        if closed.declaring_trait != dispatch.declaring_trait
            || closed.template != dispatch.requirement
            || !program.requirement_application_matches_scope(dispatch.contract_signature, &scope)
        {
            return reject("closed-public-contract-scope");
        }
    }
    let requirement = dispatch.requirement;
    let Some(application_trait) = program.trait_definition_by_symbol(application.trait_definition)
    else {
        return reject("missing-application-trait");
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
        return reject("requirement-not-declared-by-selected-trait");
    }
    let Some(declaration) = program.trait_definition_by_symbol(dispatch.declaring_trait) else {
        return reject("missing-requirement-owner");
    };
    if program
        .trait_machine_signatures(declaration)
        .iter()
        .filter(|signature| signature.symbol == requirement)
        .count()
        != 1
    {
        return reject("requirement-roster-not-unique");
    }
    let Some(selected) = program.conformance_by_symbol(application.declaration) else {
        return reject("missing-selected-conformance");
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
        return reject("selected-row-not-unique");
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

/// Replay declaration context independently of invocation or named-map capture.
/// Exact parent authority may itself be a lexical application at a prior rung.
pub(crate) fn lexical_declaration_binding_is_exact(
    program: &dyn crate::CheckingInput,
    declaration: &typed_trees::typed_trees::LexicalMachineDeclaration,
) -> Result<(), &'static str> {
    if !declaration.parent_application.is_valid() {
        return Err("lexical declaration lost its parent application");
    }
    if program.executable_application_template(declaration.instance)? != Some(declaration.template)
        || program.executable_application_template(declaration.parent_instance)?
            != Some(declaration.parent_template)
    {
        return Err("lexical declaration has ambiguous or unrelated executable ownership");
    }
    if program.symbols.get(declaration.template).parent != declaration.parent_template
        || program.symbols.get(declaration.instance).parent != declaration.parent_instance
        || program.symbols.get(declaration.instance).generated_from != declaration.template
    {
        return Err("lexical declaration does not retain exact source and applied ancestry");
    }
    if program.executable_binding_application(declaration.parent_instance)?
        != Some(declaration.parent_application)
    {
        return Err("lexical declaration substitution differs from its parent's exact application");
    }
    let template = program
        .machine_by_symbol(declaration.template)
        .ok_or("lexical declaration lost its source body")?;
    let instance = program
        .machine_by_symbol(declaration.instance)
        .ok_or("lexical declaration lost its applied body")?;
    let parameters = program.machine_type_parameters(template);
    let applied = program.machine_type_parameters(instance);
    if template.lifetime_parameters != instance.lifetime_parameters
        || parameters.len() != applied.len()
        || !parameters.iter().zip(applied).all(|(source, copied)| {
            source.name == copied.name
                && source.bounds == copied.bounds
                && std::mem::discriminant(&source.kind) == std::mem::discriminant(&copied.kind)
                && program.symbols.get(copied.symbol).parent == instance.symbol
                && program.symbols.get(copied.symbol).generated_from == source.symbol
        })
    {
        return Err("lexical declaration changed its independent invocation telescope");
    }
    let states = program
        .captured_machine_states
        .span(declaration.states)
        .ok_or("lexical declaration has stale entry correspondence")?;
    let originals = program.machine_states(template);
    let applied = program.machine_states(instance);
    if states.len() != originals.len()
        || states.len() != applied.len()
        || !states
            .iter()
            .zip(originals.iter().zip(applied))
            .all(|(mapping, (source, copied))| {
                mapping.template == source.symbol
                    && mapping.instance == copied.symbol
                    && program.symbols.get(copied.symbol).generated_from == source.symbol
            })
    {
        return Err("lexical declaration has incomplete entry correspondence");
    }
    for (source, copied) in originals.iter().zip(applied) {
        let parameters = program.state_parameters(source);
        let copied = program.state_parameters(copied);
        if parameters.len() != copied.len()
            || !parameters.iter().zip(copied).all(|(source, copied)| {
                source.is_mutable == copied.is_mutable
                    && source.relevance == copied.relevance
                    && program.symbols.get(copied.symbol).generated_from == source.symbol
            })
        {
            return Err("lexical declaration added or captured runtime arguments");
        }
    }
    Ok(())
}
