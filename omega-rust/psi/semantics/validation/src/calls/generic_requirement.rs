//! The same generic receiver requirement selected by ordinary call validation.

use super::*;

/// Recover the public requirement of an exact evidence-child call in its
/// declaring generic machine. This grants no concrete realization identity.
pub fn named_conformance_target_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    target: symbols::SymbolHandle,
) -> Result<
    Option<(
        symbols::SymbolHandle,
        &'program typed_trees::signature::StateSignature,
    )>,
    String,
> {
    if !target.is_valid() {
        return Ok(None);
    }
    for bound in &machine.conformance_bounds {
        let Some(binder) = bound.binder else {
            continue;
        };
        if program
            .symbols
            .child_handles(binder)
            .is_some_and(|mut children| children.any(|child| child == target))
        {
            return named_conformance_requirement(program, machine, binder, target).map(
                |requirement| {
                    requirement.map(|requirement| {
                        (requirement.trait_definition.symbol, requirement.signature)
                    })
                },
            );
        }
    }
    Ok(None)
}

pub(super) fn named_conformance_receiver(
    program: &TypedTrees,
    machine: &Machine,
    receiver: symbols::SymbolHandle,
) -> bool {
    receiver.is_valid()
        && program.symbols.get(receiver).kind == symbols::SymbolKind::ConformanceParameter
        && machine
            .conformance_bounds
            .iter()
            .any(|bound| bound.binder == Some(receiver))
}

pub(super) fn expression_conformance_receiver(
    program: &TypedTrees,
    machine: &Machine,
    receiver: ExpressionHandle,
) -> symbols::SymbolHandle {
    let ExpressionNode::Name(path) = program.expression_table.expression(receiver) else {
        return symbols::SymbolHandle::invalid();
    };
    if named_conformance_receiver(program, machine, path.symbol) {
        path.symbol
    } else {
        symbols::SymbolHandle::invalid()
    }
}

/// Evidence selects a requirement namespace, not an implicit runtime receiver.
/// Rejoin its exact child to the bound's reachable, unambiguous requirement.
pub(super) fn named_conformance_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    receiver: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
) -> Result<Option<crate::traits::GenericBoundRequirement<'program>>, String> {
    if !named_conformance_receiver(program, machine, receiver) {
        return Ok(None);
    }
    let failure = || {
        format!(
            "machine `{}` conformance-evidence call has no unique exact requirement",
            machine.name
        )
    };
    let mut bounds = machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder == Some(receiver));
    let bound = bounds.next().ok_or_else(failure)?;
    if bounds.next().is_some()
        || !target.is_valid()
        || program.symbols.get(target).kind != symbols::SymbolKind::State
        || !program
            .symbols
            .child_handles(receiver)
            .is_some_and(|mut children| children.any(|child| child == target))
    {
        return Err(failure());
    }
    let carrier = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == bound.carrier)
        .ok_or_else(failure)?;
    let mut selected = None;
    for trait_definition in program.traits() {
        if crate::traits::arguments_for_declaring_trait(
            program,
            carrier,
            &bound.arguments,
            trait_definition.symbol,
            &mut Vec::new(),
        )
        .is_none()
        {
            continue;
        }
        for signature in program.trait_machine_signatures(trait_definition) {
            if signature.name.as_str() != program.symbols.name(target) {
                continue;
            }
            if selected.is_some() {
                return Err(failure());
            }
            selected = Some(crate::traits::GenericBoundRequirement {
                signature,
                trait_definition,
                bound,
            });
        }
    }
    selected.map(Some).ok_or_else(failure)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_named_conformance_arguments(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    requirement: &crate::traits::GenericBoundRequirement<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parameters = program
        .state_signature_parameters(requirement.signature)
        .to_vec();
    for parameter in &mut parameters {
        parameter.is_self = false;
    }
    let Some(carrier) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == requirement.bound.carrier)
    else {
        return;
    };
    let Some(inherited_arguments) = crate::traits::arguments_for_declaring_trait(
        program,
        carrier,
        &requirement.bound.arguments,
        requirement.trait_definition.symbol,
        &mut Vec::new(),
    ) else {
        return;
    };
    let mut bound = requirement.bound.clone();
    bound.arguments = inherited_arguments;
    let requirement = crate::traits::GenericBoundRequirement {
        signature: requirement.signature,
        trait_definition: requirement.trait_definition,
        bound: &bound,
    };
    // The declared trait argument is the expected destination even when an
    // actual is a literal or computation rather than a typed storage place.
    // Reuse ordinary literal landing and narrowing against that exact handle.
    let trait_parameters = program.trait_type_parameters(requirement.trait_definition);
    for parameter in &mut parameters {
        let TypeReferenceNode::Named { symbol, .. } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            continue;
        };
        if let Some(argument) = trait_parameters
            .iter()
            .position(|candidate| candidate.symbol == *symbol)
            .and_then(|ordinal| requirement.bound.arguments.get(ordinal))
        {
            parameter.type_reference = *argument;
        }
    }
    validate_call_arguments_handles(
        program,
        machine,
        state,
        value_env,
        arguments,
        requirement.signature.name.as_str(),
        &parameters,
        None,
        writable_roots,
        diagnostics,
    );
    for (argument, parameter) in arguments.iter().zip(&parameters) {
        let Some(actual) = declared_place_type(program, machine, state, *argument) else {
            continue;
        };
        // Shared call checking above verifies reference access. Compare the
        // referents here, substituting Self by the exact retained subject.
        let required = crate::places::unwrapped_type_reference(program, parameter.type_reference)
            .unwrap_or(parameter.type_reference);
        if !crate::traits::named_conformance_argument_matches(
            program,
            actual,
            required,
            &requirement,
        ) {
            diagnostics.push(Diagnostic::error(format!("argument `{}` for conformance requirement `{}::{}` does not match its bound subject and trait arguments", parameter.name, requirement.trait_definition.name, requirement.signature.name)));
        }
    }
}

/// Resolve only the declared generic receiver-bound channel. The checked
/// borrow gate uses this exact owner when a raw call has no concrete target;
/// this does not instantiate the signature or grant an executable call.
pub fn generic_bound_call_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &State,
    receiver: &str,
    target: &str,
) -> Result<Option<&'program typed_trees::signature::StateSignature>, String> {
    let Some(receiver_type) = declared_receiver_type_reference(program, machine, state, receiver)
    else {
        return Ok(None);
    };
    crate::traits::generic_bound_requirement_call(program, machine, receiver_type, target)
        .map(|requirement| requirement.map(|requirement| requirement.signature))
}

/// Value receivers retain their complete declared place, including nested
/// member/index projections. Use precisely the owner used by value-call
/// validation rather than reducing a receiver to its final member spelling.
pub fn generic_bound_value_call_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &State,
    receiver: ExpressionHandle,
    target: &str,
) -> Result<Option<&'program typed_trees::signature::StateSignature>, String> {
    let Some(receiver_type) = declared_place_type(program, machine, Some(state), receiver) else {
        return Ok(None);
    };
    crate::traits::generic_bound_requirement_call(program, machine, receiver_type, target)
        .map(|requirement| requirement.map(|requirement| requirement.signature))
}
