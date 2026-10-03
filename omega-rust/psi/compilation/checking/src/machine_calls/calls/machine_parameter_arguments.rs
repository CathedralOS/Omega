//! A static binder calls its complete declared parameter roster explicitly.
//!
//! Nominal requirements retain receiver spelling for their subject identity;
//! that spelling does not insert a receiver at a binder invocation. Only this
//! scratch call view clears it. Nominal selection and its complete effect and
//! proof contract remain with the ordinary binder admission owner.

use super::*;
use typed_trees::signature::StateSignature;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_machine_parameter_arguments(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value_environment: &ValueEnvironment,
    arguments: &[ExpressionHandle],
    signature: &StateSignature,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authored = program.state_signature_parameters(signature);
    let subject = authored
        .iter()
        .position(|parameter| {
            crate::declarations::traits::required_is_subject_parameter(program, parameter)
        })
        .and_then(|position| arguments.get(position))
        .and_then(|argument| declared_place_type(program, machine, state, *argument))
        .and_then(|carrier| {
            crate::value_custody::places::unwrapped_type_reference(program, carrier)
        })
        .unwrap_or_else(TypeReferenceHandle::invalid);
    let mut explicit = authored.to_vec();
    for parameter in &mut explicit {
        parameter.is_self = false;
    }
    validate_call_arguments_with_type_correspondence(
        program,
        machine,
        state,
        value_environment,
        arguments,
        signature.name.as_str(),
        &explicit,
        None,
        writable_roots,
        false,
        &[],
        false,
        |argument, required| {
            if !type_reference_mentions_self(program, required) {
                return argument_matches_carrier(program, argument, required);
            }
            if !subject.is_valid() {
                return false;
            }
            let required =
                crate::value_custody::places::peel_declared_constraints(program, required);
            let (actual, required) = if let ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(argument)
            {
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = program.type_reference_table.type_reference(required)
                else {
                    return false;
                };
                if borrow.access != *access
                    && !matches!(
                        (borrow.access, *access),
                        (
                            language_semantics::ReferenceAccess::Mutable,
                            language_semantics::ReferenceAccess::Shared
                        )
                    )
                {
                    return false;
                }
                let Some(actual) = declared_place_type(program, machine, state, argument) else {
                    return false;
                };
                (actual, *referee)
            } else {
                let Some(actual) = crate::value_custody::places::declared_place_type_raw(
                    program, machine, state, argument,
                ) else {
                    return false;
                };
                let actual =
                    crate::value_custody::places::peel_declared_constraints(program, actual);
                match (
                    program.type_reference_table.type_reference(actual),
                    program.type_reference_table.type_reference(required),
                ) {
                    (
                        TypeReferenceNode::Reference {
                            referee: actual,
                            access: language_semantics::ReferenceAccess::Mutable,
                            ..
                        },
                        TypeReferenceNode::Reference {
                            referee: required,
                            access: language_semantics::ReferenceAccess::Shared,
                            ..
                        },
                    ) => (*actual, *required),
                    _ => (actual, required),
                }
            };
            crate::machine_calls::machine_parameters::required_type_matches_exact_with_subject(
                program,
                actual,
                required,
                &[],
                &mut Vec::new(),
                subject,
            )
        },
        diagnostics,
    );
}
