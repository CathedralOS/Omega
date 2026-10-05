//! Publish a source local only after its exact initializer result exists.
use super::super::{CheckedUnitEffectOperationPlan, LoweringError, unsupported};
use super::CheckedTrees;
use checked_trees::CheckedArrayConstructionSource;
use checked_trees::statement::StatementNode;

/// Bind only the local whose initializer owns this exact published result.
/// Nested call/argument values in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
/// The dense result route registers the bound local in the view roster beside
/// this join when its produced type names a view family.
pub(crate) fn bind_local(
    checked: &CheckedTrees,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    declaration: terminal_psi::StructuralPlaceDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    locals: &mut Vec<crate::expression_preparation::bindings::StructuralLocalBinding>,
) -> Result<(), LoweringError> {
    let (result, expression) = match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: CheckedArrayConstructionSource::Statement,
            result,
            ..
        } => (result, None),
        // A successful call binds its exact initializer result regardless of
        // whether installation selected the callee through a boundary.
        CheckedUnitEffectOperationPlan::Call(call)
            if let Some(result) = call.structural_result() =>
        {
            let authored = crate::call_sites::locate(checked, caller_state, call.coordinate)?;
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                authored.source_site
            else {
                return Ok(());
            };
            (result, Some(expression))
        }
        _ => return Ok(()),
    };
    let (_, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(());
    };
    // Every readable returned reference publishes the same exact local-to-place
    // join. Its declared access distinguishes shared from mutable permission;
    // omitting shared carriers makes later computations lose their live source.
    // A shared `&[T]` local borrows its slice referent through the same
    // SharedBorrow custody the named-referee arms admit — `shared_borrowed_parts`
    // keeps its named restriction, so the slice view peels separately. Its
    // published identity is the peeled carrier the call result records.
    let shared_slice =
        crate::expression_preparation::source_custody::structural::shared_slice_view_referent(
            checked,
            local.type_reference,
        );
    let borrowed_access =
        crate::reference_custody::mutable_borrow_parts(checked, local.type_reference)
            .or_else(|| {
                crate::reference_custody::shared_borrow_parts(checked, local.type_reference)
            })
            .map(|(_, access)| {
                crate::expression_preparation::bindings::structural_paths::terminal_access(access)
            })
            .or_else(|| shared_slice.map(|_| terminal_psi::StructuralAccess::SharedBorrow));
    // The roster's owned-source admission is one storage judgment: an already
    // established owned value whose exact constrained type survives the
    // transfer. It classifies every producer and wrapper (plain arrays,
    // numeric-constrained and byte-predicate-domained owners, Named or
    // Constrained references) rather than per-shape combinations.
    // A returned nominal owner binds the same local home as any other owned
    // result. Its eventual cleanup is an edge obligation, not a reason to
    // omit the live source binding needed by later stores and transfers.
    let owned_contents =
        validation::has_cleanup_owned_contents(&checked.typed, local.type_reference)
            || crate::content_classes::plain_owned_with_represented_constraints(
                checked,
                local.type_reference,
            );
    if expression.is_some_and(|expression| expression != local.initial_value)
        || !(borrowed_access.is_some() || owned_contents)
    {
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
