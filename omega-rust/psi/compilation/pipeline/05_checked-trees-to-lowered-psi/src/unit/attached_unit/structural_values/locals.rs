//! Publish a source local only after its exact initializer result exists.
use super::super::{CheckedUnitEffectOperationPlan, LoweringError, unsupported};
use super::CheckedTrees;
use checked_trees::CheckedArrayConstructionSource;

/// Bind only the local whose initializer owns this exact published result.
/// Nested call/argument values in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
/// The dense result route registers the bound local in the view roster beside
/// this join when its produced type names a view family.
pub(crate) fn bind_local(
    checked: &CheckedTrees,
    operation: &CheckedUnitEffectOperationPlan,
    declaration: terminal_psi::StructuralPlaceDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    locals: &mut Vec<crate::expression_preparation::bindings::StructuralLocalBinding>,
) -> Result<(), LoweringError> {
    let result = match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: CheckedArrayConstructionSource::Statement,
            result,
            ..
        } => result,
        // A successful call binds its exact initializer result regardless of
        // whether installation selected the callee through a boundary.
        CheckedUnitEffectOperationPlan::Call(call)
            if let Some(result) = call.structural_result() =>
        {
            result
        }
        _ => return Ok(()),
    };
    let local = result.local;
    if !local.is_named() {
        return Ok(());
    }
    // Every readable returned reference publishes the same exact local-to-place
    // join. Its binding custody distinguishes shared from mutable permission;
    // omitting shared carriers makes later computations lose their live source.
    let borrowed_access =
        (local.access != checked_trees::CheckedStructuralAccess::Owned).then(|| {
            crate::expression_preparation::bindings::structural_paths::terminal_access(local.access)
        });
    // The roster's owned-source admission is one storage judgment: an already
    // established owned value whose exact constrained type survives the
    // transfer. It classifies every producer and wrapper (plain arrays,
    // numeric-constrained and byte-predicate-domained owners, Named or
    // Constrained references) rather than per-shape combinations.
    // A returned nominal owner binds the same local home as any other owned
    // result. Its eventual cleanup is an edge obligation, not a reason to
    // omit the live source binding needed by later stores and transfers.
    let owned_contents =
        validation::has_contents(
            &checked.typed,
            local.declared_type,
            validation::ContentsRequirement::CleanupOwned,
        ) || crate::content_classes::plain_owned_with_represented_constraints(
            checked,
            local.declared_type,
        ) || crate::content_classes::linear_owned_contents(checked, local.declared_type);
    if !(borrowed_access.is_some() || owned_contents) {
        return Ok(());
    }
    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = declaration.kind
    else {
        return unsupported("structural local has no exact produced carrier");
    };
    locals.push(
        crate::expression_preparation::bindings::StructuralLocalBinding {
            result_binding_ordinal: Some(result.binding_ordinal),
            root: facts::PlaceRoot::Symbol(local.symbol),
            structural_type,
            argument: terminal_psi::StructuralArgument {
                place: declaration.id,
                access: if structural_types.iter().any(|declaration| {
                    declaration.id == structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::Reference { .. }
                        )
                }) {
                    terminal_psi::StructuralAccess::Owned
                } else {
                    borrowed_access.unwrap_or(terminal_psi::StructuralAccess::Owned)
                },
                path: Vec::new(),
            },
        },
    );
    Ok(())
}
