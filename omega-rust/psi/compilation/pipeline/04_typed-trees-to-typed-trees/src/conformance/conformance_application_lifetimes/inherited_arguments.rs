//! Compare the nominal application underlying a source evidence bound.
//! Refinements constrain existing maps; they never become satisfaction targets.
//! Both sides keep their original declaration-relative operands until the
//! caller applies its own static and region telescope.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::GenericConformanceBound;
use typed_trees::trait_definition::{AppliedTraitScope, Conformance, TraitApplicationArguments};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn materialize(
    program: &mut TypedTrees,
    selected: &Conformance,
    bound: &GenericConformanceBound,
    receiving_owner: SymbolHandle,
) -> Result<(Vec<TypeReferenceHandle>, Vec<TypeReferenceHandle>), String> {
    let declaration = program
        .trait_definition_by_symbol(bound.trait_symbol)
        .ok_or("evidence bound lost its trait declaration")?;
    let mut required = AppliedTraitScope::new(
        program,
        declaration,
        TraitApplicationArguments {
            lexical_owner: receiving_owner,
            enclosing_data_applications: &[],
            arguments: &bound.arguments,
            lifetimes: &[],
        },
    )
    .ok_or("evidence bound lost its complete application")?;
    let mut active = Vec::new();
    while let Some(base) = &required.declaration.refines {
        if active.contains(&required.declaration.symbol) {
            return Err("cyclic evidence refinement application".into());
        }
        active.push(required.declaration.symbol);
        required = required
            .declaring_scope(program, base.symbol)
            .ok_or("evidence refinement has no unique nominal application")?;
    }
    let supplied = AppliedTraitScope::from_conformance(program, selected)
        .and_then(|scope| scope.declaring_scope(program, required.declaration.symbol))
        .ok_or("selected map has no unique inherited application of the required trait")?;
    let supplied = supplied.environment();
    let required = required.environment();
    let supplied = program.materialize_trait_application_arguments(&supplied)?;
    let required = program.materialize_trait_application_arguments(&required)?;
    Ok((supplied, required))
}
