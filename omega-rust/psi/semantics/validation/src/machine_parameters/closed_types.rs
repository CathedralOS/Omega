//! Read-only type substitution through already checked static-call selections.
//! This grants neither callable admission nor a returned-view loan.

use super::type_refinement::{TypeBinding, required_type_matches_exact};
use super::*;
use typed_trees::signature::StateSignature;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn closed_static_call_type_bindings(
    program: &TypedTrees,
    caller: &Machine,
    state: &State,
    signature: &StateSignature,
    selected: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    let parameters = program.state_signature_type_parameters(signature);
    // This projection supports closed type/static-machine tuples. Other
    // telescope kinds remain with their existing checking/instantiation owner.
    if parameters.iter().any(|parameter| {
        !matches!(
            parameter.kind,
            TypeParameterKind::Type | TypeParameterKind::Machine { .. }
        )
    }) {
        return None;
    }
    let generic_types = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    let requirements = parameters
        .iter()
        .filter_map(|parameter| match &parameter.kind {
            TypeParameterKind::Machine { contract } => {
                Some(machine_parameter_signature(program, contract))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if selected.is_empty() || selected.len() != requirements.len() {
        return None;
    }
    let mut bindings = Vec::<TypeBinding>::new();
    for (requirement, selected) in requirements.into_iter().zip(selected) {
        if selected.application.is_some()
            || selected.const_literal.is_some()
            || selected.evidence_projection.is_some()
            || !program
                .state_signature_type_parameters(requirement)
                .is_empty()
        {
            return None;
        }
        let (machine, entry) = machine_and_state(program, selected.symbol)?;
        if !program.machine_type_parameters(machine).is_empty() {
            return None;
        }
        let required = program.state_signature_parameters(requirement);
        let actual = program.state_parameters(entry);
        if required.len() != actual.len() {
            return None;
        }
        for (required, actual) in required.iter().zip(actual) {
            if required.is_self != actual.is_self
                || required.is_mutable != actual.is_mutable
                || required.is_const != actual.is_const
                || !required_type_matches_exact(
                    program,
                    actual.type_reference,
                    required.type_reference,
                    &generic_types,
                    &mut bindings,
                )
            {
                return None;
            }
        }
        if !required_type_matches_exact(
            program,
            entry.return_type,
            requirement.return_type,
            &generic_types,
            &mut bindings,
        ) {
            return None;
        }
    }
    let required = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if required.len() != arguments.len() {
        return None;
    }
    for (required, argument) in required.iter().zip(arguments) {
        // The ordinary place helper peels Borrow for place access. That is
        // not the type of a value passed to this call, so retain this fence.
        let actual = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(_) => return None,
            ExpressionNode::Name(path) => {
                crate::expression_types::named_value_type_reference(program, path)
            }
            _ => crate::places::declared_place_type_raw(program, caller, Some(state), *argument),
        };
        if let Some(actual) = actual {
            if !required_type_matches_exact(
                program,
                actual,
                required.type_reference,
                &generic_types,
                &mut bindings,
            ) {
                return None;
            }
        } else {
            let expected = match program
                .type_reference_table
                .type_reference(required.type_reference)
            {
                TypeReferenceNode::Named { symbol, .. } => bindings
                    .iter()
                    .find(|binding| binding.symbol == *symbol)
                    .map(|binding| binding.actual)
                    .unwrap_or(required.type_reference),
                _ => required.type_reference,
            };
            match program.expression_table.expression(*argument) {
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_) => {}
                ExpressionNode::StructLiteral(literal) => {
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(expected)
                    else {
                        return None;
                    };
                    if !symbol.is_valid()
                        || *symbol != literal.type_symbol
                        || !program.data_definitions().iter().any(|definition| {
                            definition.symbol == *symbol
                                && program.data_type_parameters(definition).is_empty()
                                && definition.lifetime_parameters.is_empty()
                        })
                    {
                        return None;
                    }
                }
                // Do not use permissive call-result fallbacks: a constructor
                // must independently name the exact closed selected carrier.
                _ => return None,
            }
            if !crate::expression_types::argument_matches_type_reference_handle(
                program, *argument, expected,
            ) {
                return None;
            }
        }
    }
    if generic_types.iter().any(|parameter| {
        !bindings
            .iter()
            .any(|binding| binding.symbol == parameter.symbol && binding.actual.is_valid())
    }) {
        return None;
    }
    Some(
        bindings
            .into_iter()
            .map(|binding| (binding.symbol, binding.actual))
            .collect(),
    )
}
