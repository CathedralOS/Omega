//! Type-parameter substitution for write-frame storage proofs.
//!
//! A boundary signature or generic carrier can name a type parameter where
//! the call site already pins a concrete storage type. These queries bind
//! parameters to caller-visible type handles and resolve substituted heads
//! without allocating type nodes. Evidence only narrows: an unbound
//! parameter stays a named leaf for the existing conservative fallbacks.

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, TypeParameter, TypeParameterKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// In-flight formal→actual type-parameter bindings. Entries are pushed while
/// a generic application expands and truncated afterward, so a binding never
/// outlives the member walk that derived it.
pub(super) type TypeBindings = Vec<(SymbolHandle, TypeReferenceHandle)>;

/// Resolve a `Named` head through the active bindings. An unbound parameter
/// keeps its named leaf for the conservative nominal fallback. Bound handles
/// are caller-side actuals; they cannot mention this instantiation's
/// parameters, but a transitive binding chain still terminates within the
/// environment's own length.
pub(super) fn substituted_head(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    for _ in 0..=bindings.len() {
        if !reference.is_valid() {
            return reference;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return reference;
        };
        let Some((_, bound)) = bindings
            .iter()
            .rev()
            .find(|(parameter, _)| parameter == symbol)
        else {
            return reference;
        };
        reference = *bound;
    }
    reference
}

/// The unique data definition behind a nominal symbol. Type parameters,
/// builtins, and ambiguous spellings leave the carrier opaque.
pub(super) fn unique_data_definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(definition)
}

/// Push one generic application's formal→actual bindings and return its data
/// definition. Every parameter must be a runtime `Type` binder with a valid
/// symbol and a supplied argument; anything else stays uninspectable, and no
/// bindings are pushed. Callers record `bindings.len()` beforehand and
/// truncate afterward so the entries cannot leak into a sibling walk.
pub(super) fn push_generic_application_bindings<'program>(
    program: &'program TypedTrees,
    base_symbol: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    bindings: &mut TypeBindings,
) -> Option<&'program DataDefinition> {
    let definition = unique_data_definition(program, base_symbol)?;
    let parameters = program.data_type_parameters(definition);
    if parameters.len() != arguments.len()
        || parameters.iter().any(|parameter| {
            !parameter.symbol.is_valid() || !matches!(parameter.kind, TypeParameterKind::Type)
        })
    {
        return None;
    }
    // Resolve each actual through the bindings already in scope so a nested
    // application records the outer instantiation, not a formal name that
    // only the sibling walk could read.
    let resolved: Vec<TypeReferenceHandle> = arguments
        .iter()
        .map(|argument| substituted_head(program, *argument, bindings))
        .collect();
    bindings.extend(
        parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .zip(resolved),
    );
    Some(definition)
}

/// Bind the declaring trait only from its exact receiver application. Call
/// arguments cannot supply a missing owner tuple or change an owner binding.
pub(super) fn trait_receiver_type_bindings(
    program: &TypedTrees,
    definition: &typed_trees::trait_definition::TraitDefinition,
    mut reference: TypeReferenceHandle,
) -> Option<TypeBindings> {
    let parameters = program.trait_type_parameters(definition);
    if parameters.is_empty() {
        return Some(Vec::new());
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

pub(super) fn signature_call_type_bindings_seeded(
    program: &TypedTrees,
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
        false,
        bindings,
    )
}

/// The same binding walk with `self` retained in the formal→actual
/// correspondence. Declaration-qualified requirement calls (`Trait::m(x)`)
/// supply `self` as an ordinary argument, so excluding it would shift every
/// remaining pairing. `Self` itself names no signature `Type` binder, so the
/// extra pair still contributes no binding.
pub(super) fn signature_call_type_bindings_with_self(
    program: &TypedTrees,
    current_machine: &Machine,
    signature: &StateSignature,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    include_self: bool,
) -> Option<TypeBindings> {
    signature_call_type_bindings_in(
        program,
        current_machine,
        signature,
        site,
        arguments,
        include_self,
        Vec::new(),
    )
}

fn signature_call_type_bindings_in(
    program: &TypedTrees,
    current_machine: &Machine,
    signature: &StateSignature,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    include_self: bool,
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
    let (state, _, _) = caller_statement_at_site(program, current_machine, site)?;
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| include_self || !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }
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
    program: &TypedTrees,
    mut formal: TypeReferenceHandle,
    mut actual: TypeReferenceHandle,
    parameters: &[TypeParameter],
    bindings: &mut TypeBindings,
) -> bool {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(formal)
    {
        formal = *base_type;
    }
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(actual)
    {
        actual = *base_type;
    }
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
