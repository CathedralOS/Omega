//! Publish a source local only after its exact initializer result exists.
use super::super::CheckedUnitEffectOperationPlan;
use checked_trees::CheckedArrayConstructionSource;

/// Bind only the local whose initializer owns this exact published result.
/// Nested call/argument values in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
/// The dense result route registers the bound local in the view roster beside
/// this join when its produced type names a view family.
pub(crate) fn local_binding(
    operation: &CheckedUnitEffectOperationPlan,
    declaration: terminal_psi::StructuralPlaceDeclaration,
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<crate::expression_preparation::bindings::StructuralLocalBinding> {
    let result = match operation {
        CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                source: CheckedArrayConstructionSource::Statement,
                result,
                ..
            },
        ) => result,
        // A successful call binds its exact initializer result regardless of
        // whether installation selected the callee through a boundary.
        CheckedUnitEffectOperationPlan::Call(call)
            if let Some(result) = call.structural_result() =>
        {
            result
        }
        _ => return None,
    };
    let local = result.local;
    if !local.is_named() {
        return None;
    }
    // Every readable returned reference publishes the same exact local-to-place
    // join. Its binding custody distinguishes shared from mutable permission;
    // omitting shared carriers makes later computations lose their live source.
    let borrowed_access =
        (local.access != checked_trees::CheckedStructuralAccess::Owned).then(|| {
            crate::expression_preparation::bindings::structural_paths::terminal_access(local.access)
        });
    // Source checking and call emission already established this exact result.
    // Its local binding is independent of cleanup multiplicity or the kinds of
    // fields it contains. Reclassifying the type here omits otherwise valid
    // owners (for example a record carrying shared descriptors), leaving later
    // observations without the home the call actually produced.
    Some(
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
    )
}
