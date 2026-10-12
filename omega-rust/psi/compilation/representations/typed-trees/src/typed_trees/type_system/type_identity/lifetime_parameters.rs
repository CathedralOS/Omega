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
    if let Some((ordinal, scope)) = scopes.iter().enumerate().next_back() {
        match scope.region_role {
            super::AppliedTypeIdentityRegionRole::Supplied => {
                let (_, argument) = scope
                    .lifetimes
                    .iter()
                    .find(|(parameter, _)| parameter == name)?;
                return applied_lifetime_parameter_identity(
                    program,
                    lexical_owner,
                    argument.as_deref()?,
                    &scopes[..ordinal],
                );
            }
            super::AppliedTypeIdentityRegionRole::Declaration => {
                let region = lifetime_parameter_identity(program, scope.declaration, name)?;
                if region.declaration != program.lifetime_declaration_owner(scope.declaration)
                    && let Some(outer) =
                        scopes[..ordinal].iter().rposition(|outer| {
                            program
                                .original_declaration_lifetime_parameter(outer.declaration, name)
                                .zip(program.original_declaration_lifetime_parameter(
                                    region.declaration,
                                    name,
                                ))
                                .is_some_and(|(left, right)| left == right)
                        })
                {
                    return applied_lifetime_parameter_identity(
                        program,
                        lexical_owner,
                        name,
                        &scopes[..=outer],
                    );
                }
                return Some(region);
            }
        }
    }
    lifetime_parameter_identity(program, lexical_owner, name)
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
