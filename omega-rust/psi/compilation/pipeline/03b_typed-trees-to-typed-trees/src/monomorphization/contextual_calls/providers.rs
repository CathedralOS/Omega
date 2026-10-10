//! Derive a selected provider's own complete tuple from an applied contract.
//! Reuse the ordinary clone, cache identity and retained tuple. Generic expected
//! telescopes are universal schemas, never permission to choose one instance.

use super::super::{
    Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees, candidate, saved_calls,
};
use typed_trees::signature::StateSignature;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(super) fn close(
    program: &mut TypedTrees,
    selected: SymbolHandle,
    expected: &StateSignature,
    caller_types: &[(SymbolHandle, TypeReferenceHandle)],
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
    if !program.state_signature_type_parameters(expected).is_empty() {
        return Ok(None);
    }
    let Some(machine_index) = program.machines().iter().position(|machine| {
        machine.has_specialization_parameters()
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == selected)
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
        .find(|state| state.symbol == selected)
        .cloned();
    let Some(state) = state else {
        return Ok(None);
    };
    let actual_types = program
        .state_parameters(&state)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect::<Vec<_>>();
    let expected_types = program
        .state_signature_parameters(expected)
        .iter()
        .map(|parameter| parameter.type_reference)
        .collect::<Vec<_>>();
    if actual_types.len() != expected_types.len() {
        return Ok(None);
    }
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    for (actual, required) in actual_types
        .into_iter()
        .chain([state.return_type])
        .zip(expected_types.into_iter().chain([expected.return_type]))
    {
        let required = program
            .apply_callable_type(required, caller_types)
            .map_err(|message| vec![Diagnostic::error(message)])?;
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
    // A symbolic caller still owns its generic schema. Executable cloning waits
    // until every inferred operand is closed, including compound types.
    if provider
        .type_bindings
        .iter()
        .chain(&provider.const_bindings)
        .flatten()
        .any(|reference| super::static_operands::is_open(program, *reference))
    {
        return Ok(None);
    }
    if provider.conflicted
        || provider.type_bindings.iter().any(Option::is_none)
        || provider.const_bindings.iter().any(Option::is_none)
        || provider.machine_bindings.iter().any(Option::is_none)
        || provider.evidence_bindings.iter().any(Option::is_none)
    {
        return Ok(None);
    }
    let existing = saved_calls::candidate_instance(program, program, &provider)
        .map(|instance| instance.instance);
    if let Some(existing) = existing {
        let ordinal = provider
            .template
            .state_symbols
            .iter()
            .position(|symbol| *symbol == selected);
        return Ok(program
            .machine_by_symbol(existing)
            .and_then(|machine| {
                ordinal.and_then(|ordinal| program.machine_states(machine).get(ordinal))
            })
            .map(|state| state.symbol));
    }
    let Some(identity) = normalized_machine_identity(program, &program.machines()[machine_index])
    else {
        return Err(vec![Diagnostic::error(
            "contextual machine application has no normalized declaration identity",
        )]);
    };
    let commitment = accepted_template_commitment(program, machine_index);
    let ordinal = program
        .machine_specializations
        .iter()
        .filter(|instance| instance.template == provider.template.template_symbol)
        .count();
    let states = super::super::body_cloning::clone_specialized_machine(
        None, program, &provider, ordinal, identity, commitment,
    )?;
    Ok(states
        .into_iter()
        .find(|(original, _)| *original == selected)
        .map(|(_, concrete)| concrete))
}
