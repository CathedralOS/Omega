//! Original declaration roles and zero-telescope interface completion.
//!
//! Ordinary ancestors remain contract inputs; only boundary declarations
//! contribute executable boundary members. A declaration with no interface or
//! invocation binders already owns its complete signature. Other members need
//! their retained instantiated application, never a guessed sibling tuple.

use crate::TypedTrees;
use crate::trait_definition::AppliedTraitScope;
use symbols::SymbolHandle;

/// The original boundary member roster, preserving each declaring owner.
/// This is declaration inventory, not application or provider admission.
pub fn boundary_interface_members(
    owner: &TypedTrees,
    selected_interface: SymbolHandle,
) -> Option<Vec<(SymbolHandle, SymbolHandle)>> {
    let selected = owner.trait_definition_by_symbol(selected_interface)?;
    let mut members = Vec::new();
    AppliedTraitScope::abstract_scope(selected).visit(owner, &mut |scope| {
        if scope.declaration.is_boundary {
            members.extend(
                owner
                    .trait_machine_signatures(scope.declaration)
                    .iter()
                    .map(|signature| (scope.declaration.symbol, signature.symbol)),
            );
        }
    });
    members.sort_by_key(|(declaration, member)| {
        (
            declaration.arena_index(),
            declaration.generation(),
            member.arena_index(),
            member.generation(),
        )
    });
    members.dedup();
    Some(members)
}

/// Authenticate an original sibling that needs no generated public header.
/// Both the selected and declaring interface must have zero outer telescopes;
/// a closed child inheriting an applied generic parent still needs that
/// parent's instantiated signature. Invocation-static operands are separate.
pub fn original_requirement_is_complete(
    owner: &TypedTrees,
    selected_interface: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    let Some(selected) = owner.trait_definition_by_symbol(selected_interface) else {
        return false;
    };
    let Some((declaring, signature)) = owner.trait_machine_signature(requirement) else {
        return false;
    };
    selected.is_boundary
        && declaring.is_boundary
        && owner.trait_type_parameters(selected).is_empty()
        && selected.conformance_bounds.is_empty()
        && selected.lifetime_parameters.is_empty()
        && owner.trait_type_parameters(declaring).is_empty()
        && declaring.conformance_bounds.is_empty()
        && declaring.lifetime_parameters.is_empty()
        && owner.state_signature_type_parameters(signature).is_empty()
        && signature.lifetime_parameters.is_empty()
        && boundary_interface_members(owner, selected_interface)
            .is_some_and(|members| members.contains(&(declaring.symbol, requirement)))
}
