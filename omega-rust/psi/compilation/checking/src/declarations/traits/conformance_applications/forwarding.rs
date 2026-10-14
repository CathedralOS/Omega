//! Evidence forwarding preserves an exact authored promise, not a visible map.
//! Callers supply their own application-aware argument equality; declaration
//! lookup and refinement authority are shared with trait implementation slots.

use symbols::SymbolKind;
use typed_trees::{
    data::TypeParameterKind,
    type_identity::{
        AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
        applied_index_identity, applied_type_identity_in_scope,
    },
};
use typed_trees::{expression::StaticMachineArgument, machine::GenericConformanceBound};

pub(crate) fn validate_forwarded_argument(
    program: &dyn crate::CheckingInput,
    required: &GenericConformanceBound,
    selected: &StaticMachineArgument,
    arguments_match: impl FnOnce(&GenericConformanceBound) -> bool,
) -> Result<(), String> {
    if !selected.symbol.is_valid()
        || program.symbols.get(selected.symbol).kind != SymbolKind::ConformanceParameter
    {
        return Err("forwarded evidence must name a declared implementation binder".to_owned());
    }
    if selected.application.is_some() {
        return Err("an implementation evidence binder is already selected and takes no application arguments".to_owned());
    }
    let actual = program
        .traits()
        .iter()
        .flat_map(|definition| definition.conformance_bounds.iter())
        .chain(
            program
                .machines()
                .iter()
                .flat_map(|machine| machine.conformance_bounds.iter()),
        )
        .find(|actual| actual.binder == selected.symbol)
        .ok_or_else(|| "forwarded implementation binder has no declared contract".to_owned())?;
    let demanded = crate::refinement_fit::resolve_bound_carrier(program, required.trait_symbol);
    let promised = crate::refinement_fit::resolve_bound_carrier(program, actual.trait_symbol);
    // A bare base promise does not grant its caller a stronger refinement.
    // Runtime carriers and a subsequently chosen concrete map cannot supply it.
    if promised.base != demanded.base
        || (demanded.refinement.is_some() && actual.trait_symbol != required.trait_symbol)
        || !arguments_match(actual)
    {
        return Err(format!(
            "forwarded binder `{}` does not guarantee the exact `{}` trait application",
            actual.binder_name, required.trait_name
        ));
    }
    Ok(())
}

pub(crate) fn applied_bound_argument_identities(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    receiving_owner: symbols::SymbolHandle,
    argument_owner: symbols::SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<Vec<NormalizedTypeIdentity>> {
    let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let identity = |arguments: &[typed_trees::types::TypeReferenceHandle],
                    scopes: &[AppliedTypeIdentityScope<'_>]| {
        applied_trait_argument_identities(
            program,
            carrier.base,
            arguments,
            receiving_owner,
            argument_owner,
            scopes,
        )
    };
    if let Some((refinement, base)) = carrier
        .refinement
        .and_then(|refinement| refinement.refines.as_ref().map(|base| (refinement, base)))
    {
        if program.trait_type_parameters(refinement).len() != bound.arguments.len() {
            return None;
        }
        let head = program
            .trait_type_parameters(refinement)
            .iter()
            .zip(&bound.arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    if parameter.kind == TypeParameterKind::Type {
                        IndexIdentityArgument::TypeArgument(*argument)
                    } else {
                        IndexIdentityArgument::TypeReference(*argument)
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut scopes = scopes.to_vec();
        scopes.push(AppliedTypeIdentityScope {
            declaration: refinement.symbol,
            arguments: &head,
            lifetimes: &[],
        });
        applied_trait_argument_identities(
            program,
            carrier.base,
            program
                .type_reference_table
                .type_reference_handles(base.arguments),
            refinement.symbol,
            argument_owner,
            &scopes,
        )
    } else {
        identity(&bound.arguments, scopes)
    }
}

/// The receiving telescope decides whether syntax denotes a type or a value.
/// Both required and selected applications must use that same role before
/// comparison, including a const supplied through a named implementation map.
pub(crate) fn applied_trait_argument_identities(
    program: &dyn crate::CheckingInput,
    declaration: symbols::SymbolHandle,
    arguments: &[typed_trees::types::TypeReferenceHandle],
    receiving_owner: symbols::SymbolHandle,
    argument_owner: symbols::SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<Vec<NormalizedTypeIdentity>> {
    let definition = program.trait_definition_by_symbol(declaration)?;
    let parameters = program.trait_type_parameters(definition);
    if parameters.len() != arguments.len() {
        return None;
    }
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| match parameter.kind {
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                applied_index_identity(
                    program.typed_program(),
                    IndexIdentityArgument::TypeReference(*argument),
                    argument_owner,
                    scopes,
                )
            }
            _ => applied_type_identity_in_scope(
                program.typed_program(),
                *argument,
                receiving_owner,
                argument_owner,
                scopes,
            ),
        })
        .collect()
}
