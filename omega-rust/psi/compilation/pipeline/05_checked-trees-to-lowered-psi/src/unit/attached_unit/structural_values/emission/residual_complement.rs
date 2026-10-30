//! The maximal residual complement a projected owned move leaves in its root:
//! every untouched subtree of the root's finite records and arrays, in
//! reverse declaration order at each record level. Checking's partial-affine
//! complement walk (`validation::reconstruct`) derives it; a
//! root with no partial-affine decomposition has no residual discard route.

use crate::LoweringError;
use checked_trees::{
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};

/// A root whose entire structural complement is gone has no storage to carry
/// through a private continuation. Structural emptiness does not retire any
/// nominal hook on a surviving ancestor of a moved subtree. A hook on the
/// moved subtree itself travels with that subtree and is not owed here.
pub(super) fn is_empty(
    checked: &checked_trees::CheckedTrees,
    state: symbols::SymbolHandle,
    statement: usize,
    symbol: symbols::SymbolHandle,
    vacancies: &[Vec<facts::PlaceSegment>],
) -> Result<bool, LoweringError> {
    let root_type = validation::canonical_place_type_reference(
        &checked.typed,
        state,
        statement,
        &validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        },
    )
    .ok_or(LoweringError::Unsupported(
        "residual owner has no exact type",
    ))?;
    let has_nominal_hook = |reference| {
        let reference = checked
            .type_reference_table
            .unconstrained_type_reference(reference);
        // Arrays and references do not own their elements' or referents'
        // nominal hooks. Inspect this value, not recursively its type symbol.
        let nominal = match checked.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. }
            | typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => *symbol,
            _ => return false,
        };
        checked.machines().iter().any(|machine| {
            validation::nominal_cleanup_owner(&checked.typed, machine.symbol) == Some(nominal)
        })
    };
    if has_nominal_hook(root_type) {
        return Ok(false);
    }
    for vacancy in vacancies {
        for ancestor_length in 1..vacancy.len() {
            let ancestor_path = &vacancy[..ancestor_length];
            // A case selects a namespace, not another owned value. Its sum
            // owner was checked at the previous prefix; payload fields below
            // it resume the ordinary value ancestry.
            if matches!(ancestor_path.last(), Some(facts::PlaceSegment::Case { .. })) {
                continue;
            }
            let (_, ancestor) =
                validation::structural_owned_path(&checked.typed, root_type, ancestor_path)
                    .map_err(LoweringError::Unsupported)?;
            if has_nominal_hook(ancestor) {
                return Ok(false);
            }
        }
    }
    let root_identity =
        validation::stored_structural_type_identity(&checked.typed, root_type, &[], &[]);
    let moved = vacancies
        .iter()
        .map(|path| {
            let (path, leaf) = validation::structural_owned_path(&checked.typed, root_type, path)
                .map_err(LoweringError::Unsupported)?;
            Ok((
                path,
                validation::stored_structural_type_identity(&checked.typed, leaf, &[], &[]),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let moved = moved
        .iter()
        .map(|(path, identity)| (path.as_slice(), *identity))
        .collect::<Vec<_>>();
    Ok(complement(
        &checked.facts.flow.terminal_unit_effects.structural_types,
        &CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        root_identity,
        &moved,
    )?
    .is_empty())
}

/// The residual discards `moved_paths` leave in a root of type `root_type`.
pub(super) fn complement(
    types: &[CheckedUnitStructuralTypePlan],
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: typed_trees::type_identity::TypeIdentityId,
    moved_paths: &[(
        &[CheckedUnitStructuralPathSegment],
        typed_trees::type_identity::TypeIdentityId,
    )],
) -> Result<Vec<CheckedUnitPartialAffineDiscardPlan>, LoweringError> {
    let borrowed = types
        .iter()
        .map(|declaration| (declaration.identity, declaration))
        .collect();
    validation::reconstruct(&borrowed, source, root_type, moved_paths, &[], usize::MAX).ok_or(
        LoweringError::Unsupported(
            "projected move's residual complement has no partial-affine discard route",
        ),
    )
}
