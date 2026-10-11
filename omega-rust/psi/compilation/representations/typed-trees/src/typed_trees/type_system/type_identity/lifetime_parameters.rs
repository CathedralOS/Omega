//! Resolve declaration-relative regions before comparing an applied type or
//! admitting a static lifetime operand. An unused region still needs a lexical
//! declaration; searching only the type's reference leaves would miss it.

use crate::TypedTrees;
use symbols::SymbolHandle;

/// Local semantic identity, not a portable artifact commitment or a loan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifetimeParameterIdentity {
    pub declaration: SymbolHandle,
    pub ordinal: usize,
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
    if let Some((ordinal, scope)) = scopes.iter().enumerate().next_back()
        && let Some((_, argument)) = scope
            .lifetimes
            .iter()
            .find(|(parameter, _)| parameter == name)
    {
        return applied_lifetime_parameter_identity(
            program,
            lexical_owner,
            argument,
            &scopes[..ordinal],
        );
    }
    lifetime_parameter_identity(
        program,
        scopes
            .last()
            .map_or(lexical_owner, |scope| scope.declaration),
        name,
    )
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
        let parameters = program
            .machine_by_symbol(owner)
            .map(|owner| &owner.lifetime_parameters)
            .or_else(|| {
                program
                    .conformance_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .trait_definition_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .data_definition_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .trait_machine_signature(owner)
                    .map(|(_, signature)| &signature.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .retained_static_machine_contract(owner)
                    .map(|contract| &contract.signature().lifetime_parameters)
            });
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
        owner = program.lifetime_declaration_owner(program.symbols.get(owner).parent);
    }
    None
}
