//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case and record identities remain structural; field and dispatch
//! operands share scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.

use super::*;
use checked_trees::CheckedArrayConstructionSource;
use checked_trees::statement::StatementNode;

mod emission;
mod record;
pub(crate) mod source_custody;

pub(crate) use emission::emit;
pub(crate) use record::emit_record;

/// Bind only the local whose initializer owns this exact published result.
/// Nested calls and argument constructors in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
pub(super) fn bind_local(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
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
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate, result, ..
        } => {
            let authored = crate::call_source_custody::authored::locate_source(
                checked,
                machine.state,
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
    let (_, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(());
    };
    if expression.is_some_and(|expression| expression != local.initial_value)
        || (!validation::is_closed_primitive_array_type(&checked.typed, local.type_reference)
            && !(expression.is_some()
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
    if local.is_mutable
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
            access: terminal_psi::StructuralAccess::Owned,
            path: Vec::new(),
        },
    ));
    Ok(())
}
