//! Evidence forwarding preserves an exact authored promise, not a visible map.
//! Callers supply their own application-aware argument equality; declaration
//! lookup and refinement authority are shared with trait implementation slots.

use symbols::SymbolKind;
use typed_trees::{
    data::TypeParameterKind,
    type_identity::{
        AppliedTypeIdentityScope, IndexIdentityArgument, LifetimeParameterIdentity,
        NormalizedTypeIdentity, applied_index_identity_in_region_scope,
        applied_type_identity_in_region_scope,
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
        .callable_conformance_bound(program.symbols.get(selected.symbol).parent, selected.symbol)
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TraitApplicationIdentity {
    arguments: Vec<NormalizedTypeIdentity>,
    lifetimes: Vec<typed_trees::type_identity::LifetimeParameterIdentity>,
}

pub(crate) fn applied_bound_argument_identities(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    receiving_owner: symbols::SymbolHandle,
    argument_owner: symbols::SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<TraitApplicationIdentity> {
    applied_bound_argument_identities_in_region_scope(
        program,
        bound,
        receiving_owner,
        argument_owner,
        scopes,
        &[],
    )
}

pub(crate) fn applied_bound_argument_identities_in_region_scope(
    program: &dyn crate::CheckingInput,
    bound: &GenericConformanceBound,
    receiving_owner: symbols::SymbolHandle,
    argument_owner: symbols::SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    regions: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<TraitApplicationIdentity> {
    let carrier = crate::refinement_fit::resolve_bound_carrier(program, bound.trait_symbol);
    let root = typed_trees::trait_definition::AppliedTraitScope::new(
        program.typed_program(),
        program.trait_definition_by_symbol(bound.trait_symbol)?,
        typed_trees::trait_definition::TraitApplicationArguments {
            lexical_owner: receiving_owner,
            enclosing_data_applications: &[],
            arguments: &bound.arguments,
            lifetimes: &bound.lifetime_arguments,
        },
    )?;
    let scope = root.declaring_scope(program.typed_program(), carrier.base)?;
    applied_scope_argument_identities_in_region_scope(
        program,
        &scope,
        argument_owner,
        scopes,
        regions,
    )
}

/// Static operands and explicit region slots belong to one complete inherited
/// application. A lifetime unused by a method still distinguishes its promise.
/// Reuse each parameter's preceding simultaneous scopes; never borrow a
/// selected provider's region merely because its spelling matches a receiver.
pub(crate) fn applied_scope_argument_identities(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    argument_owner: symbols::SymbolHandle,
    outer: &[AppliedTypeIdentityScope<'_>],
) -> Option<TraitApplicationIdentity> {
    applied_scope_argument_identities_in_region_scope(
        program,
        application,
        argument_owner,
        outer,
        &[],
    )
}

fn applied_scope_argument_identities_in_region_scope(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    argument_owner: symbols::SymbolHandle,
    outer: &[AppliedTypeIdentityScope<'_>],
    regions: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<TraitApplicationIdentity> {
    application.with_identity_scopes(program.typed_program(), |inner| {
        let identities = program
            .trait_type_parameters(application.declaration)
            .iter()
            .map(|parameter| {
                let (argument, prefix) = application.parameter_application(parameter.symbol)?;
                let mut scopes = outer.to_vec();
                scopes.extend_from_slice(&inner[..prefix]);
                match parameter.kind {
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                        applied_index_identity_in_region_scope(
                            program.typed_program(),
                            IndexIdentityArgument::TypeReference(argument),
                            argument_owner,
                            &scopes,
                            regions,
                        )
                    }
                    _ => applied_type_identity_in_region_scope(
                        program.typed_program(),
                        argument,
                        application.argument_owner,
                        argument_owner,
                        &scopes,
                        regions,
                    ),
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let mut scopes = outer.to_vec();
        scopes.extend_from_slice(inner);
        let lifetimes = application
            .declaration
            .lifetime_parameters
            .iter()
            .map(|parameter| {
                // Start with the receiving declaration/ordinal. A caller's rigid
                // same-spelled binder must not bypass this exact application row.
                let slot = typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    application.declaration.symbol,
                    parameter.as_str(),
                )?;
                typed_trees::type_identity::apply_resolved_lifetime_parameter_identity(
                    program.typed_program(),
                    slot,
                    argument_owner,
                    &scopes,
                )
                .map(|(argument, _)| {
                    // The caller already checked this frame. Apply its exact
                    // correspondence once, after resolving the supplied slot.
                    regions
                        .iter()
                        .find(|(parameter, _)| *parameter == argument)
                        .map_or(argument, |(_, resolved)| *resolved)
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(TraitApplicationIdentity {
            arguments: identities,
            lifetimes,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        TypedTrees, data::DataDefinition, name::Identifier, type_aliases::TypeAliasDefinition,
    };

    #[test]
    fn forwarded_promises_include_data_and_alias_headers_without_weakening_checks() {
        for kind in [SymbolKind::Data, SymbolKind::TypeAlias] {
            let mut builder = SymbolTableBuilder::default();
            let owner = builder.insert_root(kind, SymbolNameRef::Static("Owner"));
            let binder = builder
                .insert_children(
                    owner,
                    [(
                        SymbolKind::ConformanceParameter,
                        SymbolNameRef::Static("Policy"),
                    )],
                )
                .start();
            let required_trait =
                builder.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Required"));
            let other_trait =
                builder.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Other"));
            let mut program = TypedTrees::default();
            program.symbols = builder.finish();
            let promised = GenericConformanceBound {
                binder,
                binder_name: Identifier::generated("Policy"),
                trait_symbol: required_trait,
                trait_name: Identifier::generated("Required"),
                lifetime_arguments: Vec::new(),
                arguments: Vec::new(),
            };
            if kind == SymbolKind::TypeAlias {
                program.push_type_alias_definition(TypeAliasDefinition {
                    symbol: owner,
                    conformance_bounds: vec![promised.clone()],
                    ..Default::default()
                });
            } else {
                program.push_data_definition(DataDefinition {
                    symbol: owner,
                    conformance_bounds: vec![promised.clone()],
                    ..Default::default()
                });
            }
            let selected = StaticMachineArgument {
                symbol: binder,
                ..Default::default()
            };
            assert!(
                validate_forwarded_argument(&program, &promised, &selected, |actual| actual
                    == &promised)
                .is_ok()
            );
            assert!(
                validate_forwarded_argument(&program, &promised, &selected, |_| false).is_err()
            );
            let mut incompatible = promised.clone();
            incompatible.trait_symbol = other_trait;
            incompatible.trait_name = Identifier::generated("Other");
            assert!(
                validate_forwarded_argument(&program, &incompatible, &selected, |_| panic!(
                    "trait mismatch must precede tuple comparison"
                ))
                .is_err()
            );
            let mut applied = selected.clone();
            applied.application =
                Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                    member: Default::default(),
                    lifetime_arguments: Box::new([]),
                    arguments: Box::new([]),
                }));
            assert!(
                validate_forwarded_argument(&program, &promised, &applied, |_| panic!(
                    "an applied binder must refuse first"
                ))
                .unwrap_err()
                .contains("already selected")
            );
            let mut missing = TypedTrees::default();
            missing.symbols = program.symbols.clone();
            assert!(
                validate_forwarded_argument(&missing, &promised, &selected, |_| panic!(
                    "a missing promise must refuse first"
                ))
                .unwrap_err()
                .contains("no declared contract")
            );
        }
    }
}
