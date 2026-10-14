//! Resolve declaration-relative regions before comparing an applied type or
//! admitting a static lifetime operand. An unused region still needs a lexical
//! declaration; searching only the type's reference leaves would miss it.

use super::constraint_identity::{atom, compound};
use crate::TypedTrees;
use symbols::SymbolHandle;

/// Local semantic identity, not a portable artifact commitment or a loan.
/// The default's zero declaration is the arena absence row, not ordinal zero
/// in any live declaration's telescope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LifetimeParameterIdentity {
    pub declaration: SymbolHandle,
    pub ordinal: usize,
}

/// Project an already resolved region into declaration-relative coordinates.
/// A callable owner needs its overload identity, not just its path. Package
/// custody and declaration kind remain independent of binder spelling; a local
/// arena coordinate is never written into this projection.
pub fn portable_lifetime_parameter_identity(
    program: &TypedTrees,
    region: LifetimeParameterIdentity,
) -> Option<String> {
    lifetime_parameter_name(program, region)?;
    let owner = region.declaration;
    let (kind, declaration) = if let Some(machine) = program.machine_by_symbol(owner) {
        ("machine", program.machine_callable_identity_text(machine)?)
    } else if let Some((declaration, requirement)) = program.trait_machine_signature(owner) {
        (
            "requirement",
            program.trait_requirement_callable_identity_text(declaration, requirement),
        )
    } else if program.conformance_by_symbol(owner).is_some() {
        ("implementation", program.symbols.display_path(owner, "::"))
    } else if let Some(declaration) = program.trait_definition_by_symbol(owner) {
        ("trait", program.trait_declaration_path(declaration))
    } else if program.data_definition_by_symbol(owner).is_some() {
        ("data", program.symbols.display_path(owner, "::"))
    } else {
        // An unjoined static-machine contract has no portable declaration
        // coordinate yet. A same-spelled source path cannot supply one.
        return None;
    };
    if declaration.is_empty() {
        return None;
    }
    let coordinate = super::identity_context::TypeIdentityContext {
        qualification: super::TypeIdentityQualification::PackageQualified,
        ..Default::default()
    }
    .qualify_non_binder_name(program, owner, declaration);
    Some(compound(
        "region",
        [
            atom("kind", kind),
            atom("declaration", &coordinate),
            atom("ordinal", &region.ordinal.to_string()),
        ],
    ))
}

/// Resolve a captured region through simultaneous applications without turning
/// its local semantic coordinate into text. An operand belongs to the preceding
/// scopes, not the receiving telescope. Invocation-local binders are resolved
/// separately by the callable judgment before it asks this captured query.
pub fn applied_lifetime_parameter_identity(
    program: &TypedTrees,
    lexical_owner: SymbolHandle,
    name: &str,
    scopes: &[super::AppliedTypeIdentityScope<'_>],
) -> Option<LifetimeParameterIdentity> {
    applied_lifetime_parameter_identity_with_supplier(
        program,
        lexical_owner,
        lexical_owner,
        name,
        scopes,
    )
    .map(|(identity, _)| identity)
}

/// A retained receiving binder and its incoming operands have distinct custody.
/// Resolve the former before switching to the latter's lexical declaration.
/// The flag records an actual retained substitution, never a name's presence
/// in an unrelated frame or the availability of a similarly named region.
pub fn applied_lifetime_parameter_identity_with_supplier(
    program: &TypedTrees,
    lexical_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    name: &str,
    scopes: &[super::AppliedTypeIdentityScope<'_>],
) -> Option<(LifetimeParameterIdentity, bool)> {
    let lexical_declaration = program.lifetime_declaration_owner(lexical_owner);
    if let Some(region) = lifetime_parameter_identity(program, lexical_owner, name)
        && region.declaration == lexical_declaration
        && !scopes.iter().any(|scope| {
            program.lifetime_declaration_owner(scope.declaration) == lexical_declaration
        })
    {
        // An invocation-local binder outside the supplied capture frames is
        // rigid. An outer map with the same spelling cannot capture it.
        return Some((region, false));
    }
    // Applications form a lexical stack. A final trait frame may introduce
    // no regions while an earlier map supplies a capture; stopping at that
    // final frame would erase the map's declared argument. A local unsupplied
    // binder remains rigid and shadows every outer application.
    for (ordinal, scope) in scopes.iter().enumerate().rev() {
        if let Some((_, argument)) = scope
            .lifetimes
            .iter()
            .find(|(parameter, _)| parameter == name)
        {
            return applied_lifetime_parameter_identity(
                program,
                argument_owner,
                argument,
                &scopes[..ordinal],
            )
            .map(|identity| (identity, true));
        }
        if let Some(region) = lifetime_parameter_identity(program, scope.declaration, name)
            && region.declaration == program.lifetime_declaration_owner(scope.declaration)
        {
            return Some((region, false));
        }
    }
    lifetime_parameter_identity(program, lexical_owner, name).map(|identity| (identity, false))
}

/// Find a region in the supplied lexical telescope, including captured outer
/// declarations. Executable copies retain their authored declaration's regions.
/// A same-spelled region elsewhere in the program cannot satisfy this query.
pub fn lifetime_parameter_identity(
    program: &TypedTrees,
    lexical_owner: SymbolHandle,
    name: &str,
) -> Option<LifetimeParameterIdentity> {
    let mut owner = program.lifetime_declaration_owner(lexical_owner);
    while owner.is_valid() {
        let parameters = declaration_lifetimes(program, owner);
        if let Some(ordinal) = parameters.and_then(|parameters| {
            parameters
                .iter()
                .position(|parameter| parameter.as_str() == name)
        }) {
            return Some(LifetimeParameterIdentity {
                declaration: owner,
                ordinal,
            });
        }
        owner = program.lifetime_declaration_owner(program.declaration_scope_parent(owner));
    }
    None
}

/// Recover a payload spelling only after its exact declaration/ordinal has
/// been resolved. The spelling is not the identity and must retain this owner
/// when it is copied into another type node.
pub fn lifetime_parameter_name(
    program: &TypedTrees,
    region: LifetimeParameterIdentity,
) -> Option<&crate::name::Identifier> {
    declaration_lifetimes(program, region.declaration)?.get(region.ordinal)
}

fn declaration_lifetimes(
    program: &TypedTrees,
    owner: SymbolHandle,
) -> Option<&[crate::name::Identifier]> {
    program
        .machine_by_symbol(owner)
        .map(|owner| owner.lifetime_parameters.as_slice())
        .or_else(|| {
            program
                .conformance_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.as_slice())
        })
        .or_else(|| {
            program
                .trait_definition_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.as_slice())
        })
        .or_else(|| {
            program
                .data_definition_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.as_slice())
        })
        .or_else(|| {
            program
                .trait_machine_signature(owner)
                .map(|(_, signature)| signature.lifetime_parameters.as_slice())
        })
        .or_else(|| {
            program
                .retained_static_machine_contract(owner)
                .map(|contract| contract.signature().lifetime_parameters.as_slice())
        })
}
