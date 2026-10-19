//! Close a sealed placed primitive onto its existing plan-issued operation.
//! Ordinary generic specialization owns inference and retained applications;
//! this catalog branch supplies a pre-existing exact target rather than cloning
//! another bodyless declaration. All source/loan checks therefore see the same
//! target as provider settlement and physical access lowering.

use typed_trees::typed_trees::{PlacedAccessPrimitive, PlacedScalarAccessorDeclaration};

use super::{CallSelection, Candidate, Diagnostic, SymbolHandle, TypedTrees};

/// Elaborate complete catalog calls before the ordinary body-cloning gate.
/// Incomplete inference remains in the fixed point. Neither the caller's
/// syntax nor the presence of unrelated generic binders changes catalog supply.
pub(super) fn select_calls(
    program: &mut TypedTrees,
    candidates: &[Candidate],
    selections: &mut Vec<CallSelection>,
    changed_owners: &mut Vec<SymbolHandle>,
) -> Result<bool, Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    for selection in selections
        .iter()
        .filter(|selection| selection.is_complete())
    {
        let candidate = super::candidate_for_selection(
            program,
            &candidates[selection.candidate_index],
            selection,
        );
        let Some(states) = select(None, program, &candidate)? else {
            continue;
        };
        let Some((_, entry)) = states
            .iter()
            .find(|(original, _)| *original == selection.callee_symbol)
        else {
            return Err(vec![Diagnostic::error(
                "placed primitive call lost its exact entry selection",
            )]);
        };
        rewrites.push((selection.site, selection.lexical_owner, *entry));
    }
    selections.retain(|selection| !rewrites.iter().any(|(site, _, _)| *site == selection.site));
    for (site, owner, entry) in &rewrites {
        super::body_rewriting::rewrite_selected_call(program, *site, *entry, &[]);
        changed_owners.push(*owner);
    }
    Ok(!rewrites.is_empty())
}

pub(super) fn select(
    source: Option<&dyn validation::CheckingInput>,
    program: &TypedTrees,
    candidate: &Candidate,
) -> Result<Option<Vec<(SymbolHandle, SymbolHandle)>>, Vec<Diagnostic>> {
    let declarations = source.map_or(program, |source| source.typed_program());
    let Some(operation) =
        PlacedAccessPrimitive::from_declaration(declarations, candidate.template.template_symbol)
            .map_err(|diagnostic| vec![diagnostic])?
    else {
        return Ok(None);
    };
    let [Some(accessor), Some(value)] = candidate.type_bindings.as_slice() else {
        return Err(vec![Diagnostic::error(
            "placed primitive lost its complete accessor/value application",
        )]);
    };
    let target = operation
        .select_target(declarations, *accessor, *value)
        .map_err(|diagnostic| vec![diagnostic])?;
    if source.is_some() {
        let original =
            PlacedScalarAccessorDeclaration::authenticate(declarations, target.state_symbol)
                .map_err(|diagnostic| vec![diagnostic])?;
        let retained = PlacedScalarAccessorDeclaration::authenticate(program, target.state_symbol)
            .map_err(|diagnostic| vec![diagnostic])?;
        if original != retained {
            return Err(vec![Diagnostic::error(
                "placed primitive target is not retained in the destination placement roster",
            )]);
        }
    }
    // This is catalog selection, not a cloned machine. Recording a clone origin
    // would change the field target's visibility and authored satisfaction edge.
    // The caller already retained its inferred tuple in the source application;
    // contextual machine operands retain their own provider application and
    // selected entry. Immutable checking rejoins those occurrences to this target.
    Ok(Some(vec![(
        candidate.template.state_symbols[0],
        target.state_symbol,
    )]))
}
