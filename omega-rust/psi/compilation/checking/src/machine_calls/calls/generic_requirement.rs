//! Requirement calls through an evidence binder (`Order::rank(...)` under
//! `Order satisfies Ranked<Element>`). The binder selects the requirement
//! namespace; the call's arguments are ordinary explicit values.

use super::{
    Diagnostic, ExpressionHandle, ExpressionNode, Machine, State, StateParameter, ValueEnvironment,
    WritableRoots, argument_matches_carrier, declared_place_type, declared_reference_access,
    validate_call_arguments,
};

/// The public signature of a retained requirement selection. An evidence child
/// is not itself a trait declaration: rejoin its declaring machine's bound and
/// complete application before exposing formals or guarantees. All consumers
/// see the same substituted contract; none may replace it with a provider body.
pub fn selected_requirement_signature(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<(
    &typed_trees::trait_definition::TraitDefinition,
    &typed_trees::signature::StateSignature,
)> {
    if let Some(selected) = program.trait_machine_signature(target) {
        return Some(selected);
    }
    if !target.is_valid() {
        return None;
    }
    let binder = program.symbols.get(target).parent;
    if !binder.is_valid()
        || program.symbols.get(binder).kind != symbols::SymbolKind::ConformanceParameter
    {
        return None;
    }
    let machine = program.machine_by_symbol(program.symbols.get(binder).parent)?;
    // Ordinary call admission reports malformed or ambiguous evidence bounds.
    // This read-only query supplies no contract when that admission cannot join.
    let requirement = named_conformance_target_requirement(program, machine, target).ok()??;
    let application = &requirement.application;
    let signature = program
        .closed_requirement_applications()
        .find(|closed| {
            closed.template == requirement.signature.symbol
                && closed.local_arguments.is_empty()
                && program
                    .requirement_application_matches_scope(closed.signature.symbol, application)
        })
        .map(|closed| closed.signature)
        .or_else(|| program.applied_callable_signature(application, requirement.signature.symbol))
        .or_else(|| {
            (application.substitutions.is_empty()
                && application.declaration.lifetime_parameters.is_empty()
                && requirement.signature.type_parameters.is_empty())
            .then_some(requirement.signature)
        })?;
    Some((application.declaration, signature))
}

/// The requirement already selected its trait telescope. Raw signature handles
/// still name that telescope (`&mut Message`, not the caller's `&mut M`), so a
/// stored reference must compare under its bound rather than against raw ids.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_requirement_call_arguments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    parameters: &[StateParameter],
    requirement: &crate::declarations::traits::GenericBoundRequirement<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments(
        program,
        machine,
        state,
        value_environment,
        arguments,
        requirement.signature.name.as_str(),
        requirement.signature.name.as_str(),
        parameters,
        writable_roots.statements,
        false,
        &[],
        |argument, required| {
            let raw_actual = crate::value_custody::places::declared_place_type_raw(
                program, machine, state, argument,
            );
            // Explicit borrows and non-reference destinations retain ordinary
            // syntax/type matching. This correspondence is for stored refs;
            // it never exempts their access or subsequent domain checks.
            if !matches!(
                program.expression_table.expression(argument),
                ExpressionNode::Borrow(_)
            ) && declared_reference_access(program, required).is_some()
                && let Some(actual) = raw_actual
                && declared_reference_access(program, actual).is_some()
            {
                // Ordinary calls permit mutable-to-shared attenuation. Trait
                // signature matching is invariant in access, so compare the
                // referents explicitly for that one permitted call relation.
                let (actual, required) =
                    crate::value_custody::type_shapes::attenuated_reference_pair(
                        program, actual, required,
                    );
                return crate::declarations::traits::named_conformance_argument_matches(
                    program,
                    actual,
                    required,
                    requirement,
                );
            }
            argument_matches_carrier(program, machine.symbol, argument, required, &[])
        },
        diagnostics,
    );
}

/// Recover the public requirement of an exact evidence-child call in its
/// declaring generic machine. This grants no concrete realization identity.
pub fn named_conformance_target_requirement<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    target: symbols::SymbolHandle,
) -> Result<Option<crate::declarations::traits::GenericBoundRequirement<'program>>, String> {
    if !target.is_valid() {
        return Ok(None);
    }
    for bound in &machine.conformance_bounds {
        let binder = bound.binder;
        if program
            .symbols
            .child_handles(binder)
            .is_some_and(|mut children| children.any(|child| child == target))
        {
            return named_conformance_requirement(program, machine, binder, target);
        }
    }
    Ok(None)
}

pub(super) fn named_conformance_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    receiver: symbols::SymbolHandle,
) -> bool {
    receiver.is_valid()
        && program.symbols.get(receiver).kind == symbols::SymbolKind::ConformanceParameter
        && machine
            .conformance_bounds
            .iter()
            .any(|bound| bound.binder == receiver)
}

pub(super) fn expression_conformance_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    receiver: ExpressionHandle,
) -> symbols::SymbolHandle {
    let Some(path) = program.expression_table.name_path_node(receiver) else {
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
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    receiver: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
) -> Result<Option<crate::declarations::traits::GenericBoundRequirement<'program>>, String> {
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
        .filter(|bound| bound.binder == receiver);
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
        .trait_definition_by_symbol(bound.trait_symbol)
        .ok_or_else(failure)?;
    let root = typed_trees::trait_definition::AppliedTraitScope::new(
        program,
        carrier,
        typed_trees::trait_definition::TraitApplicationArguments {
            lexical_owner: machine.symbol,
            enclosing_data_applications: &[],
            arguments: &bound.arguments,
            lifetimes: &[],
        },
    )
    .ok_or_else(failure)?;
    let mut selected: Option<crate::declarations::traits::GenericBoundRequirement<'program>> = None;
    let mut ambiguous = false;
    root.visit(program, &mut |application| {
        for signature in program.trait_machine_signatures(application.declaration) {
            if signature.name.as_str() != program.symbols.name(target) {
                continue;
            }
            if let Some(known) = &selected {
                ambiguous |= known.signature.symbol != signature.symbol
                    || !known.application.same_application(program, application);
                continue;
            }
            selected = Some(crate::declarations::traits::GenericBoundRequirement {
                signature,
                application: application.clone(),
            });
        }
    });
    if ambiguous {
        return Err(failure());
    }
    selected.map(Some).ok_or_else(failure)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_named_conformance_arguments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    requirement: &crate::declarations::traits::GenericBoundRequirement<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parameters = program
        .state_signature_parameters(requirement.signature)
        .to_vec();
    // The declared trait argument is the expected destination even when an
    // actual is a literal or computation rather than a typed storage place.
    // Reuse ordinary literal landing and narrowing against that exact handle.
    for parameter in &mut parameters {
        parameter.type_reference = requirement
            .application
            .argument_reference(program, parameter.type_reference);
    }
    validate_requirement_call_arguments(
        program,
        machine,
        state,
        value_environment,
        arguments,
        &parameters,
        requirement,
        writable_roots,
        diagnostics,
    );
    for (argument, parameter) in arguments.iter().zip(&parameters) {
        let Some(actual) = declared_place_type(program, machine, state, *argument) else {
            continue;
        };
        // Shared call checking above verifies reference access. Compare the
        // referents here under the binder's trait arguments.
        let required = crate::value_custody::places::unwrapped_type_reference(
            program,
            parameter.type_reference,
        )
        .unwrap_or(parameter.type_reference);
        if !crate::declarations::traits::named_conformance_argument_matches(
            program,
            actual,
            required,
            requirement,
        ) {
            diagnostics.push(Diagnostic::error(format!("argument `{}` for conformance requirement `{}::{}` does not match the binder's trait arguments", parameter.name, requirement.application.declaration.name, requirement.signature.name)));
        }
    }
}

/// Why a call through a named implementation (`Impl::member(...)`) selected no
/// row: the implementation has no row for that requirement, or several
/// inherited rows share its spelling. Resolution leaves both unresolved
/// rather than choosing one.
pub(super) fn unresolved_implementation_call(
    program: &dyn crate::CheckingInput,
    implementation: symbols::SymbolHandle,
    target: &str,
) -> String {
    let name = program.symbols.name(implementation);
    let rows = program
        .conformance_by_symbol(implementation)
        .map(|conformance| conformance.rows.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|row| row.requirement_name.as_str() == target)
        .map(|row| format!("`{}::{}`", row.declaring_trait_name, row.requirement_name))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        format!("implementation `{name}` has no row for requirement `{target}`")
    } else {
        format!(
            "implementation `{name}` has {} rows named `{target}` ({}); a call through the implementation must select exactly one",
            rows.len(),
            rows.join(", ")
        )
    }
}
