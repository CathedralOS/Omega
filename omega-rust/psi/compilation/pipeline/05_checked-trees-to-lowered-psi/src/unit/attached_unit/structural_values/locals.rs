//! Publish a source local only after its exact initializer result exists.
use super::super::{CheckedUnitEffectOperationPlan, LoweringError, PlaceId, unsupported};
use super::CheckedTrees;
use checked_trees::CheckedArrayConstructionSource;
use checked_trees::statement::StatementNode;

/// Bind only the local whose initializer owns this exact published result.
/// Nested call/argument values in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
pub(crate) fn bind_local(
    checked: &CheckedTrees,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    place: PlaceId,
    locals: &mut Vec<(symbols::SymbolHandle, terminal_psi::StructuralArgument)>,
) -> Result<(), LoweringError> {
    bind_local_with_view(
        checked,
        caller_state,
        operation,
        place,
        None,
        locals,
        None,
        &[],
    )
}

/// The same publication, additionally registering an established view local
/// when the produced place carries a view family.
pub(crate) fn bind_local_with_view(
    checked: &CheckedTrees,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    place: PlaceId,
    structural_type: Option<semantic_vocabulary::StructuralTypeId>,
    locals: &mut Vec<(symbols::SymbolHandle, terminal_psi::StructuralArgument)>,
    view_locals: Option<
        &mut Vec<crate::expression_preparation::bindings::view_locals::ViewLocalBinding>,
    >,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let (result, expression) = match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: CheckedArrayConstructionSource::Statement,
            result,
            ..
        } => (result, None),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate, result, ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate, result, ..
        } => {
            let authored = crate::emission::call_source_custody::authored::locate_source(
                checked,
                caller_state,
                *coordinate,
            )?;
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                authored.source_site
            else {
                return Ok(());
            };
            (result, Some(expression))
        }
        _ => return Ok(()),
    };
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(());
    };
    // A `&mut` view local carries the referent's storage as affine loan
    // custody: its binding joins as a mutable borrow of the published place,
    // the same registration the composed route gives `&mut` call results.
    let mutable_view = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .is_some();
    // A shared `& T` view local binds its referent carrier the same affine way
    // the composed route does: the call result records the peeled carrier
    // identity, so admission and the identity check both ride the referee.
    let shared_view_referee = validation::reference_result_custody::shared_place_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .map(|(referee, _)| referee);
    if expression.is_some_and(|expression| expression != local.initial_value)
        || !(mutable_view
            || shared_view_referee.is_some()
            || validation::is_closed_primitive_array_type(&checked.typed, local.type_reference)
            || (expression.is_some()
                && matches!(
                    checked
                        .type_reference_table
                        .type_reference(local.type_reference),
                    checked_trees::types::TypeReferenceNode::Named { .. }
                )
                && validation::has_plain_owned_contents_with_numeric_constraints(
                    checked,
                    local.type_reference,
                )))
    {
        return Ok(());
    }
    let declared = match shared_view_referee {
        Some(referee) => crate::unit::attached_unit::structural_carrier_type(checked, referee)?,
        None => local.type_reference,
    };
    if (local.is_mutable && !super::plain_record(checked, local.type_reference))
        || !local.symbol.is_valid()
        || locals.iter().any(|(symbol, _)| *symbol == local.symbol)
        || checked.normalized_type_identity(declared).as_str() != result.type_identity
    {
        return unsupported("structural local differs from its published initializer binding");
    }
    locals.push((
        local.symbol,
        terminal_psi::StructuralArgument {
            place,
            access: if mutable_view {
                terminal_psi::StructuralAccess::MutableBorrow
            } else if shared_view_referee.is_some() {
                terminal_psi::StructuralAccess::SharedBorrow
            } else {
                terminal_psi::StructuralAccess::Owned
            },
            path: Vec::new(),
        },
    ));
    // A view-carried result joins the view-local roster exactly as the
    // composed route registers it, so later `local.len` and element reads
    // observe the published place.
    if let (Some(structural_type), Some(view_locals)) = (structural_type, view_locals)
        && let Some(carrier) = crate::expression_preparation::bindings::view_locals::ViewCarrier::of(
            structural_type,
            structural_types,
        )
    {
        view_locals.push(
            crate::expression_preparation::bindings::view_locals::ViewLocalBinding {
                symbol: local.symbol,
                place,
                structural_type,
                carrier,
            },
        );
    }
    Ok(())
}
