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
            let authored = crate::emission::call_source_custody::authored::locate_source(
                checked,
                caller_state,
                call.coordinate,
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
    // Every readable returned reference publishes the same exact local-to-place
    // join. Its declared access distinguishes shared from mutable permission;
    // omitting shared carriers makes later computations lose their live source.
    let borrowed_access = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .or_else(|| {
        validation::reference_result_custody::shared_borrowed_parts(
            &checked.typed,
            local.type_reference,
        )
    })
    .map(|(_, access)| {
        crate::expression_preparation::bindings::structural_paths::terminal_access(access)
    });
    let owned_array =
        validation::is_closed_primitive_array_type(&checked.typed, local.type_reference);
    if expression.is_some_and(|expression| expression != local.initial_value)
        || !(borrowed_access.is_some()
            || owned_array
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
    // Initialized arrays and records both own the published storage. Binding
    // that home does not freeze its contents or lend it; later writes, copies
    // and loans independently rejoin their authored access. Reference carriers
    // still retain their separate immutable-binding requirement.
    if (local.is_mutable && !(owned_array || super::plain_record(checked, local.type_reference)))
        || !local.symbol.is_valid()
        || locals.iter().any(|(symbol, _)| *symbol == local.symbol)
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("structural local differs from its published initializer binding");
    }
    locals.push((
        local.symbol,
        terminal_psi::StructuralArgument {
            place,
            access: borrowed_access.unwrap_or(terminal_psi::StructuralAccess::Owned),
            path: Vec::new(),
        },
    ));
    Ok(())
}
