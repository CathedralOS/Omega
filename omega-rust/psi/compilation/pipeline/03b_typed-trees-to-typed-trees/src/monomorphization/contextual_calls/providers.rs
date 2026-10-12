//! Derive a selected provider's own complete tuple from an applied contract.
//! Reuse the ordinary clone, cache identity and retained tuple. Generic expected
//! telescopes are universal schemas, never permission to choose one instance.

mod explicit_arguments;
mod lifetime_arguments;

use super::super::{
    CallSite, Diagnostic, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
    candidate, saved_calls,
};
use typed_trees::signature::StateSignature;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(super) fn close(
    program: &mut TypedTrees,
    selection: &StaticMachineArgument,
    demand: SymbolHandle,
    expected: &StateSignature,
    caller_types: &[(SymbolHandle, TypeReferenceHandle)],
    site: CallSite,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    ordinal: usize,
) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
    let selected = selection.symbol;
    // Executable rewriting consumes nested tuples. Admission always reads
    // the occurrence's retained authored provider, including erased regions,
    // rather than treating a private instance as a new source declaration.
    let source_selection = program
        .expression_table
        .static_machine_application_arguments(application)
        .iter()
        .filter(|argument| super::invocations::is_machine(program, argument.symbol))
        .nth(ordinal)
        .cloned()
        .unwrap_or_else(|| selection.clone());
    let source_selected = source_selection.symbol;
    if !program.state_signature_type_parameters(expected).is_empty() {
        return Ok(None);
    }
    let Some(machine_index) = program.machines().iter().position(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == source_selected)
    }) else {
        return Ok(None);
    };
    let mut provider = candidate::from_machine(program, machine_index);
    // The tuple's operands come from this source edge, not the private clone.
    // Retaining its scope matters even for closed types containing lifetimes.
    provider.binding_application = application;
    let state = program
        .machine_states(&program.machines()[machine_index])
        .iter()
        .find(|state| state.symbol == source_selected)
        .cloned();
    let Some(state) = state else {
        return Ok(None);
    };
    // Expected operands belong to the original invocation telescope. Apply
    // its retained caller correspondence before inferring the provider tuple.
    let caller_lifetimes = program
        .expression_table
        .static_machine_application_lifetimes(application)
        .iter()
        // A nested expected callable owns fresh invocation binders. Shield
        // its telescope just as the shared full-signature copier does; an
        // outer same-spelled row supplies no actual for this universal binder.
        .filter(|binding| !expected.lifetime_parameters.contains(&binding.parameter))
        .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
        .collect::<Vec<_>>();
    let expected_types = program
        .state_signature_parameters(expected)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect::<Vec<_>>();
    let mut applied_expected_types = Vec::with_capacity(expected_types.len());
    for required in &expected_types {
        applied_expected_types.push(
            program
                .apply_callable_type(*required, caller_types, &caller_lifetimes, expected.symbol)
                .map_err(|message| vec![Diagnostic::error(message)])?,
        );
    }
    let lifetimes = lifetime_arguments::bind(
        program,
        source_selected,
        &source_selection,
        application,
        expected,
        &applied_expected_types,
    )?;
    if let Some(explicit) = &source_selection.application {
        explicit_arguments::bind(
            program,
            &mut provider,
            &state,
            source_selected,
            site,
            &explicit.arguments,
        )?;
    }
    let actual_types = program
        .state_parameters(&state)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect::<Vec<_>>();
    if actual_types.len() != expected_types.len() {
        return Ok(None);
    }
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    let applied_return = program
        .apply_callable_type(
            expected.return_type,
            caller_types,
            &caller_lifetimes,
            expected.symbol,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
    for (actual, required) in actual_types
        .into_iter()
        .chain([state.return_type])
        .zip(applied_expected_types.into_iter().chain([applied_return]))
    {
        super::super::infer_static_bindings(
            program,
            actual,
            required,
            &provider.template.type_parameters,
            &provider.template.const_parameters,
            None,
            0,
            &mut type_proposals,
            &mut const_proposals,
        );
    }
    for (_, ordinal, actual) in type_proposals {
        match provider.type_bindings[ordinal] {
            None => provider.type_bindings[ordinal] = Some(actual),
            Some(previous)
                if program.normalized_type_identity_text(previous)
                    != program.normalized_type_identity_text(actual) =>
            {
                provider.conflicted = true
            }
            Some(_) => {}
        }
    }
    for (_, ordinal, actual) in const_proposals {
        match provider.const_bindings[ordinal] {
            None => provider.const_bindings[ordinal] = Some(actual),
            Some(previous)
                if program.normalized_type_identity_text(previous)
                    != program.normalized_type_identity_text(actual) =>
            {
                provider.conflicted = true
            }
            Some(_) => {}
        }
    }
    if provider.conflicted
        || provider.type_bindings.iter().any(Option::is_none)
        || provider.const_bindings.iter().any(Option::is_none)
        || provider.machine_bindings.iter().any(Option::is_none)
        || provider.evidence_bindings.iter().any(Option::is_none)
    {
        return Ok(None);
    }
    let bindings = super::super::applications::candidate_bindings(&provider);
    if program.applied_machine_callable_matches(
        application,
        ordinal,
        selected,
        demand,
        &bindings,
        &lifetimes,
        expected,
        caller_types,
        program
            .expression_table
            .static_machine_application_lifetimes(application),
    ) {
        return Ok(None);
    }
    if !program.machines()[machine_index].has_specialization_parameters() {
        // A declaration with no static operands still needs occurrence-scoped
        // checking custody, but never needs a private executable clone.
        provider.binding_application = super::applications::retain_provider(
            program,
            &provider,
            &state,
            application,
            &lifetimes,
        );
        super::applications::retain_view(
            program,
            &provider,
            application,
            ordinal,
            selected,
            demand,
            expected,
            caller_types,
        )?;
        return Ok(Some(source_selected));
    }
    // Admission consumes the complete determined symbolic tuple, not the
    // original universal declaration. Executable instances still require closed
    // operands; a caller's const binder is not a cache key or a concrete value.
    if provider
        .type_bindings
        .iter()
        .chain(&provider.const_bindings)
        .flatten()
        .any(|reference| super::static_operands::is_open(program, *reference))
        || provider.machine_bindings.iter().flatten().any(|argument| {
            matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::MachineParameter
            ) || argument.application.is_some()
        })
        || provider.evidence_bindings.iter().flatten().any(|argument| {
            validation::conformance_applications::close_conformance_application(program, argument)
                .is_err()
        })
    {
        provider.binding_application = super::applications::retain_provider(
            program,
            &provider,
            &state,
            application,
            &lifetimes,
        );
        super::applications::retain_view(
            program,
            &provider,
            application,
            ordinal,
            selected,
            demand,
            expected,
            caller_types,
        )?;
        return Ok(Some(selected));
    }
    provider.binding_application =
        super::applications::retain_provider(program, &provider, &state, application, &lifetimes);
    let existing = saved_calls::candidate_instance(program, program, &provider)
        .map(|instance| instance.instance);
    if let Some(existing) = existing {
        let state_ordinal = provider
            .template
            .state_symbols
            .iter()
            .position(|symbol| *symbol == source_selected);
        let entry = program
            .machine_by_symbol(existing)
            .and_then(|machine| {
                state_ordinal.and_then(|ordinal| program.machine_states(machine).get(ordinal))
            })
            .map(|state| state.symbol);
        if let Some(entry) = entry {
            super::applications::retain_view(
                program,
                &provider,
                application,
                ordinal,
                entry,
                demand,
                expected,
                caller_types,
            )?;
        }
        return Ok(entry);
    }
    let Some(identity) = normalized_machine_identity(program, &program.machines()[machine_index])
    else {
        return Err(vec![Diagnostic::error(
            "contextual machine application has no normalized declaration identity",
        )]);
    };
    let commitment = accepted_template_commitment(program, machine_index);

    let specialization = super::super::body_cloning::clone_specialized_machine(
        None, program, &provider, identity, commitment,
    )?;
    let entry = specialization
        .state_symbols
        .into_iter()
        .find(|(original, _)| *original == source_selected)
        .map(|(_, concrete)| concrete);
    if let Some(entry) = entry {
        super::applications::retain_view(
            program,
            &provider,
            application,
            ordinal,
            entry,
            demand,
            expected,
            caller_types,
        )?;
    }
    Ok(entry)
}
