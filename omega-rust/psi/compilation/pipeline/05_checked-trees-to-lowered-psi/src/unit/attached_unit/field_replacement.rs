//! `place = value` over a structural field plans one roster: the producer of
//! the replacing value (the statement's call, or the establishment of its
//! construction), its store, and displacement/cleanup only when the destination
//! was occupied. An already-open hole has no old value to move or dispose.
//! Every member after the producer continues the producer's
//! statement; the destination and value are rejoined against the authored
//! assignment by the store's own source custody.
//!
//! `place = [e0, e1, ..]` over a fixed array of structural elements has no
//! whole-array establishment in Terminal, so it plans the same members once
//! per element: every element's establishment first, in authored order, then
//! each element's window pair at `place[k]`. `continues_array_elements` places
//! those members in the canonical operation order and
//! `validate_array_element_replacements` rejoins the roster to the authored
//! literal and destination.

use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, Multiplicity, unsupported,
};

/// Rejoin already-bound replacement storage to the name the assignment reads.
/// Parameters and locals have different registry coordinates, not different
/// assignment semantics. A local's source must name its exact earlier producer;
/// equal carrier types do not permit substituting another local or a displaced
/// value. Both straight-line and graph bodies use this check before emission.
pub(super) fn bound_value_type(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    statement: usize,
    expression: checked_trees::expression::ExpressionHandle,
    value: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Result<(String, Multiplicity), LoweringError> {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    let checked_trees::expression::ExpressionNode::Name(name) =
        checked.expression_table.expression(expression)
    else {
        return unsupported("bound field replacement has no authored whole-value name");
    };
    if name.symbol != name.head_symbol
        || !value.path.is_empty()
        || value.access != checked_trees::CheckedStructuralAccess::Owned
    {
        return unsupported("bound field replacement is not a whole owned value");
    }
    if let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = value.source {
        let parameter =
            parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "bound field replacement names no structural parameter",
                ))?;
        let authored = checked
            .state_parameters(source)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "bound field replacement parameter position is out of range",
            ))?;
        if authored.symbol != name.head_symbol
            || value.type_identity != parameter.type_identity
            || parameter.access != checked_trees::CheckedStructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return unsupported("bound field replacement parameter is not the authored value");
        }
        return Ok((parameter.type_identity.clone(), parameter.multiplicity));
    }
    let (declaration, local) = checked
        .state_local_declaration(source, name.head_symbol)
        .ok_or(LoweringError::Unsupported(
            "bound field replacement has no authored local",
        ))?;
    if declaration >= statement {
        return unsupported("bound field replacement precedes its local establishment");
    }
    let mut producers = operations.iter().filter_map(|operation| {
        // Operand establishments belong to their enclosing invocation, not
        // to the local that receives that invocation's completed result.
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                operand_source: Some(_),
                ..
            }
        ) || operation
            .call()
            .is_some_and(|call| call.coordinate.call_ordinal != 0)
        {
            return None;
        }
        operation
            .structural_result_binding()
            .filter(|result| result.statement_index as usize == declaration)
    });
    let result = producers.next().ok_or(LoweringError::Unsupported(
        "bound field replacement local has no structural establishment",
    ))?;
    let exact_source = match value.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            symbol == local.symbol
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            binding_ordinal == result.binding_ordinal
        }
        _ => false,
    };
    let carrier = super::structural_carrier_type(checked, local.type_reference)?;
    let identity = checked.normalized_type_identity(carrier).into_string();
    let multiplicity = checked.type_multiplicity(local.type_reference);
    if producers.next().is_some()
        || !exact_source
        || result.type_identity != identity
        || result.multiplicity != multiplicity
        || value.type_identity != identity
    {
        return unsupported("bound field replacement substituted its local or producer");
    }
    Ok((identity, multiplicity))
}

/// Whether this store (or its optional displacement) continues the same
/// statement's producer. Hole state is independently reconstructed by the
/// borrowed-window ledger; this checks value identity and operation order.
pub(super) fn continues(operations: &[CheckedUnitEffectOperationPlan], index: usize) -> bool {
    let store_index = match operations.get(index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { .. }) => index + 1,
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField { .. }) => index,
        _ => return false,
    };
    let Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
        statement_index,
        destination,
        value,
    }) = operations.get(store_index)
    else {
        return false;
    };
    let Some(previous_index) = store_index.checked_sub(1) else {
        return false;
    };
    let (displaced, producer_index) = match operations.get(previous_index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { result, source }) => {
            if result.statement_index != *statement_index || source != destination {
                return false;
            }
            let Some(producer_index) = previous_index.checked_sub(1) else {
                return false;
            };
            (Some(result), producer_index)
        }
        _ => (None, previous_index),
    };
    let Some(producer) = operations.get(producer_index) else {
        return false;
    };
    let produced = match producer {
        // The statement's own call with a structural result, ordinary or boundary.
        CheckedUnitEffectOperationPlan::Call(call)
            if let Some(result) = call.structural_result()
                && call.coordinate.call_ordinal == 0
                && call.coordinate.statement_index == *statement_index =>
        {
            result
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source: None,
            ..
        } if result.statement_index == *statement_index => result,
        _ => return false,
    };
    value.path.is_empty()
        && value.access == checked_trees::CheckedStructuralAccess::Owned
        && value.source_structural_result_binding_ordinal() == Some(produced.binding_ordinal)
        && value.type_identity == produced.type_identity
        && displaced.is_none_or(|moved| {
            moved.type_identity == produced.type_identity
                && moved.multiplicity == produced.multiplicity
        })
}

/// Whether the operation at `index` is the move-out or store of a
/// replacement whose value is storage the caller already holds — a
/// parameter, an owned local, or a binding produced at an earlier
/// statement — so no producer operation precedes the pair. The pair has no
/// coordinate of its own; the destination and value are rejoined by the
/// store's source custody exactly as in `continues`.
pub(super) fn continues_bound_value(
    operations: &[CheckedUnitEffectOperationPlan],
    index: usize,
) -> bool {
    let move_index = match operations.get(index) {
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { .. }) => index,
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField { .. }) => {
            let Some(previous) = index.checked_sub(1) else {
                return false;
            };
            previous
        }
        _ => return false,
    };
    let (
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField {
            result: moved,
            source,
        }),
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index,
            destination,
            value,
        }),
    ) = (operations.get(move_index), operations.get(move_index + 1))
    else {
        return false;
    };
    // The value is bound storage, not this statement's fresh result: the
    // moved-out hole's declared type is the value's own.
    matches!(
        value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
    ) && moved.statement_index == *statement_index
        && source == destination
        && value.path.is_empty()
        && value.access == checked_trees::CheckedStructuralAccess::Owned
        && moved.type_identity == value.type_identity
}

/// The authored statement a member of a field replacement roster names: an
/// establishment of the statement's own value, a move-out, a store, or the
/// continuation cleanup of a displaced value.
fn roster_statement(operation: Option<&CheckedUnitEffectOperationPlan>) -> Option<u32> {
    match operation? {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source: None,
            ..
        }
        | CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index, ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
            ..
        } if coordinate.call_ordinal == 0 => Some(coordinate.statement_index),
        _ => None,
    }
}

/// Whether the operation at `index` continues a whole array-literal
/// replacement its statement's first element establishment began: a later
/// element's establishment, or one element's move-out, store or displaced
/// cleanup. The statement's contiguous roster must be exactly the element
/// establishments followed by one window pair per element, in element order,
/// each store consuming its own element's binding. Destinations and element
/// identities are rejoined by `validate_array_element_replacements`.
pub(super) fn continues_array_elements(
    operations: &[CheckedUnitEffectOperationPlan],
    index: usize,
) -> bool {
    let Some(statement) = roster_statement(operations.get(index)) else {
        return false;
    };
    let mut start = index;
    while start > 0 && roster_statement(operations.get(start - 1)) == Some(statement) {
        start -= 1;
    }
    let mut end = index + 1;
    while roster_statement(operations.get(end)) == Some(statement) {
        end += 1;
    }
    let roster = &operations[start..end];
    let produced = roster
        .iter()
        .map_while(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                operand_source: None,
                ..
            } => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();
    if produced.is_empty() {
        return false;
    }
    let mut windows = roster[produced.len()..].iter().peekable();
    for produced in produced {
        let moved = match windows.peek() {
            Some(CheckedUnitEffectOperationPlan::MoveStructuralField { result, source }) => {
                windows.next();
                Some((result, source))
            }
            _ => None,
        };
        let Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            destination, value, ..
        }) = windows.next()
        else {
            return false;
        };
        if !value.path.is_empty()
            || value.access != checked_trees::CheckedStructuralAccess::Owned
            || value.source_structural_result_binding_ordinal() != Some(produced.binding_ordinal)
            || value.type_identity != produced.type_identity
            || moved.is_some_and(|(moved, source)| {
                source != destination
                    || moved.type_identity != produced.type_identity
                    || moved.multiplicity != produced.multiplicity
            })
        {
            return false;
        }
        if let Some(CheckedUnitEffectOperationPlan::ContinuationCleanup {
            cleanup_actions, ..
        }) = windows.peek()
        {
            let Some((moved, _)) = moved else {
                return false;
            };
            if moved.multiplicity != Multiplicity::Affine
                || !matches!(cleanup_actions.as_slice(), [discard]
                if discard.path().is_empty()
                    && discard.type_identity() == moved.type_identity
                    && *discard.source()
                        == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: moved.binding_ordinal,
                        })
            {
                return false;
            }
            windows.next();
        }
    }
    windows.next().is_none() && index > start
}
