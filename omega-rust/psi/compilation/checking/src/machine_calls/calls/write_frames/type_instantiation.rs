//! Type-parameter substitution for write-frame storage proofs.
//!
//! A boundary signature or generic carrier can name a type parameter where
//! the call site already pins a concrete storage type. These queries bind
//! parameters to caller-visible type handles and resolve substituted heads
//! without allocating type nodes. Evidence only narrows: an unbound
//! parameter stays a named leaf for the existing conservative fallbacks.

use super::caller_aliases::{CallerWriteSite, caller_statement_owner};
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::{StateParameter, StateSignature};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) use crate::type_applications::{
    DeclarationScopes, TypeBindings, push_generic_application_bindings, substituted_head,
};

/// Bind the declaring trait only from its exact receiver application. Call
/// arguments cannot supply a missing owner tuple or change an owner binding.
pub(super) fn trait_receiver_type_bindings(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::trait_definition::TraitDefinition,
    requirement: symbols::SymbolHandle,
    mut reference: TypeReferenceHandle,
) -> Option<TypeBindings> {
    let parameters = program.trait_type_parameters(definition);
    if parameters.is_empty() {
        return Some(Vec::new());
    }
    if let Some(application) = program.closed_requirement_application(requirement) {
        if application.declaring_trait != definition.symbol
            || application.trait_arguments.len() != parameters.len()
        {
            return None;
        }
        // The caller joined the exact receiver scope first. The prepared
        // callable owns deep substituted arguments, unlike a shallow inherited
        // node whose compound children can still refer to an outer binder.
        return Some(
            parameters
                .iter()
                .zip(application.trait_arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect(),
        );
    }
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if *base_symbol != definition.symbol
                    || parameters.len() != arguments.len()
                    || parameters.iter().any(|parameter| {
                        !parameter.symbol.is_valid()
                            || !matches!(parameter.kind, TypeParameterKind::Type)
                    })
                    || arguments.iter().any(|argument| {
                        !program
                            .type_reference_table
                            .contains_type_reference(*argument)
                    })
                {
                    return None;
                }
                return Some(
                    parameters
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .zip(arguments.iter().copied())
                        .collect(),
                );
            }
            _ => return None,
        }
    }
    None
}

/// The signature's declared parameters once the call's argument count
/// proves arity; callers bind or forward them positionally.
pub(super) fn signature_arity_parameters<'a>(
    program: &'a dyn crate::CheckingInput,
    signature: &StateSignature,
    arguments: &[ExpressionHandle],
) -> Option<Vec<&'a StateParameter>> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .collect::<Vec<_>>();
    (parameters.len() == arguments.len()).then_some(parameters)
}

pub(super) fn signature_call_type_bindings_seeded(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    signature: &StateSignature,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    bindings: TypeBindings,
) -> Option<TypeBindings> {
    signature_call_type_bindings_in(
        program,
        current_machine,
        signature,
        site,
        arguments,
        bindings,
    )
}

fn signature_call_type_bindings_in(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    signature: &StateSignature,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    mut bindings: TypeBindings,
) -> Option<TypeBindings> {
    let type_parameters = program.state_signature_type_parameters(signature);
    if type_parameters.is_empty() && bindings.is_empty() {
        return Some(bindings);
    }
    if type_parameters.iter().any(|parameter| {
        !parameter.symbol.is_valid() || !matches!(parameter.kind, TypeParameterKind::Type)
    }) {
        return None;
    }
    let (state, _, _) = caller_statement_owner(program, current_machine, site)?;
    let parameters = signature_arity_parameters(program, signature, arguments)?;
    for (parameter, argument) in parameters.iter().zip(arguments) {
        // An argument with no resolvable declared type contributes no
        // binding; the coverage check below still rejects a parameter left
        // unbound, and a reference parameter's route separately fails its
        // origin gate.
        let Some(actual) = crate::value_custody::places::declared_place_type_raw(
            program,
            current_machine,
            Some(state),
            *argument,
        ) else {
            continue;
        };
        if !bind_formal_type(
            program,
            parameter.type_reference,
            actual,
            type_parameters,
            &mut bindings,
        ) {
            return None;
        }
    }
    type_parameters
        .iter()
        .all(|parameter| {
            bindings
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
        })
        .then_some(bindings)
}

/// Best-effort formal→actual matching: only `parameters` members bind, and
/// only where the formal and actual shapes agree. A position that cannot be
/// matched simply produces no binding; the caller's coverage check rejects a
/// parameter left unbound. `false` reports a conflicting second binding for
/// the same parameter, which invalidates the whole instantiation.
pub(super) fn bind_formal_type(
    program: &dyn crate::CheckingInput,
    formal: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[TypeParameter],
    bindings: &mut TypeBindings,
) -> bool {
    let formal = program
        .type_reference_table
        .unconstrained_type_reference(formal);
    let actual = program
        .type_reference_table
        .unconstrained_type_reference(actual);
    match program.type_reference_table.type_reference(formal) {
        TypeReferenceNode::Named { symbol, .. }
            if bindings.iter().any(|(bound, _)| bound == symbol) =>
        {
            let bound = substituted_head(program, formal, bindings);
            crate::value_custody::type_references::type_references_match(program, bound, actual)
        }
        TypeReferenceNode::Named { symbol, .. }
            if parameters
                .iter()
                .any(|parameter| parameter.symbol == *symbol) =>
        {
            if let Some((_, bound)) = bindings.iter().rev().find(|(binding, _)| binding == symbol) {
                return crate::value_custody::type_references::type_references_match(
                    program, *bound, actual,
                );
            }
            bindings.push((*symbol, actual));
            true
        }
        TypeReferenceNode::Reference { referee, .. } => {
            // `declared_place_type_raw` already peels the borrow shell off a
            // place argument, so an actual that is not itself a reference
            // pairs with the formal's referee directly.
            let actual = match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::Reference { referee, .. } => *referee,
                _ => actual,
            };
            bind_formal_type(program, *referee, actual, parameters, bindings)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let TypeReferenceNode::Generic {
                base_symbol: actual_base,
                arguments: actual_arguments,
                ..
            } = program.type_reference_table.type_reference(actual)
            else {
                return true;
            };
            if actual_base != base_symbol {
                return true;
            }
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let actual_arguments = program
                .type_reference_table
                .type_reference_handles(*actual_arguments);
            arguments.len() == actual_arguments.len()
                && arguments
                    .iter()
                    .zip(actual_arguments)
                    .all(|(formal, actual)| {
                        bind_formal_type(program, *formal, *actual, parameters, bindings)
                    })
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::FixedArray {
                    element_type: actual_element,
                    ..
                } => bind_formal_type(
                    program,
                    *element_type,
                    *actual_element,
                    parameters,
                    bindings,
                ),
                _ => true,
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            match program.type_reference_table.type_reference(actual) {
                TypeReferenceNode::Slice {
                    element_type: actual_element,
                } => bind_formal_type(
                    program,
                    *element_type,
                    *actual_element,
                    parameters,
                    bindings,
                ),
                _ => true,
            }
        }
        _ => true,
    }
}
