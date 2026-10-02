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
            || parameter.is_self
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

/// Whether the cleanup at `index` disposes exactly the displaced value of
/// the replacement whose store immediately precedes it.
pub(super) fn continues_with_cleanup(
    operations: &[CheckedUnitEffectOperationPlan],
    index: usize,
) -> bool {
    let Some(store_index) = index.checked_sub(1) else {
        return false;
    };
    let (
        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        }),
        Some(CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index, ..
        }),
        Some(CheckedUnitEffectOperationPlan::MoveStructuralField { result: moved, .. }),
    ) = (
        operations.get(index),
        operations.get(store_index),
        store_index
            .checked_sub(1)
            .and_then(|move_index| operations.get(move_index)),
    )
    else {
        return false;
    };
    (continues(operations, store_index) || continues_bound_value(operations, store_index))
        && coordinate.statement_index == *statement_index
        && coordinate.call_ordinal == 0
        && moved.multiplicity == Multiplicity::Affine
        && matches!(affine_discards.as_slice(), [discard]
        if discard.path.is_empty()
            && discard.type_identity == moved.type_identity
            && discard.source
                == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: moved.binding_ordinal,
                })
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
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate, .. }
            if coordinate.call_ordinal == 0 =>
        {
            Some(coordinate.statement_index)
        }
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
        if let Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            affine_discards,
            ..
        }) = windows.peek()
        {
            let Some((moved, _)) = moved else {
                return false;
            };
            if moved.multiplicity != Multiplicity::Affine
                || !matches!(affine_discards.as_slice(), [discard]
                if discard.path.is_empty()
                    && discard.type_identity == moved.type_identity
                    && discard.source
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

/// The retained root of `assignment`'s value when it is a whole array literal
/// of structural elements rooted at this statement.
pub(crate) fn replaced_array_literal(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: usize,
    assignment: &checked_trees::statement::TableAssignment,
) -> Option<checked_trees::CheckedStructuralValueHandle> {
    let plans = &checked.facts.values.structural_values;
    let root = plans.root_for_expression(state, u32::try_from(ordinal).ok()?, assignment.value)?;
    matches!(
        plans.nodes.get(root.root).kind,
        checked_trees::CheckedStructuralValueKind::FixedArray { .. }
    )
    .then_some(root.root)
}

/// One non-array element of a whole structural array literal: its literal
/// indices below the destination, its retained value node, its authored
/// occurrence and its declared type.
pub(crate) struct ArrayLiteralLeaf {
    pub(crate) indices: Vec<u64>,
    pub(crate) value: checked_trees::CheckedStructuralValueHandle,
    pub(crate) expression: checked_trees::expression::ExpressionHandle,
    pub(crate) type_reference: checked_trees::types::TypeReferenceHandle,
}

/// The leaves of the retained array literal `value` in row-major order,
/// rejoined to the authored literal `expression` and to `declared`, which
/// must be an exact literal extent at every nested level. A nested literal
/// contributes its own leaves under all of their indices.
pub(crate) fn array_literal_leaves(
    checked: &CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
    expression: checked_trees::expression::ExpressionHandle,
    declared: checked_trees::types::TypeReferenceHandle,
) -> Option<Vec<ArrayLiteralLeaf>> {
    fn collect(
        checked: &CheckedTrees,
        value: checked_trees::CheckedStructuralValueHandle,
        expression: checked_trees::expression::ExpressionHandle,
        declared: checked_trees::types::TypeReferenceHandle,
        indices: &mut Vec<u64>,
        leaves: &mut Vec<ArrayLiteralLeaf>,
    ) -> Option<()> {
        let plans = &checked.facts.values.structural_values;
        let node = plans.nodes.get(value);
        let checked_trees::CheckedStructuralValueKind::FixedArray { elements } = &node.kind else {
            return None;
        };
        let checked_trees::expression::ExpressionNode::ArrayLiteral(literal) =
            checked.expression_table.expression(expression)
        else {
            return None;
        };
        let checked_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: checked_trees::types::FixedArrayLength::Literal(length),
        } = checked.type_reference_table.type_reference(declared)
        else {
            return None;
        };
        let authored = checked.expression_table.expression_handles(*literal);
        if node.expression != expression
            || authored.len() != elements.len()
            || *length != elements.len()
        {
            return None;
        }
        for (ordinal, (&element, &authored)) in elements.iter().zip(authored).enumerate() {
            indices.push(u64::try_from(ordinal).ok()?);
            if matches!(
                plans.nodes.get(element).kind,
                checked_trees::CheckedStructuralValueKind::FixedArray { .. }
            ) {
                collect(checked, element, authored, *element_type, indices, leaves)?;
            } else {
                leaves.push(ArrayLiteralLeaf {
                    indices: indices.clone(),
                    value: element,
                    expression: authored,
                    type_reference: *element_type,
                });
            }
            indices.pop();
        }
        Some(())
    }
    let mut leaves = Vec::new();
    collect(
        checked,
        value,
        expression,
        declared,
        &mut Vec::new(),
        &mut leaves,
    )?;
    Some(leaves)
}

/// Rejoin `place = [e0, e1, ..]` where `place` is a fixed array of structural
/// elements beneath an exclusive borrowed parameter. Terminal has no whole
/// structural-array establishment, so the checked sequencer plans the literal
/// as its leaves' own replacements, in this order: every leaf's establishment
/// in authored row-major order, binding the replacing values; then, per leaf,
/// `MoveStructuralField` binding the displaced old element and opening the
/// window at `place[i]..[k]` (absent when that element's hole is already
/// open), `StoreStructuralField` closing it with the leaf's value, and for a
/// displaced affine element the continuation discarding it. Every leaf is
/// evaluated before the first replacement, and no leaf may be dropped,
/// repeated or stored elsewhere.
pub(crate) fn validate_array_element_replacements(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    ordinal: usize,
    assignment: &checked_trees::statement::TableAssignment,
    roster: &[&CheckedUnitEffectOperationPlan],
    literal: checked_trees::CheckedStructuralValueHandle,
) -> Result<(), LoweringError> {
    let (owner, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    // The destination is the literal's exact declared array: every dimension a
    // literal extent matching the roster, with no qualification the element
    // stores could not carry.
    let declared =
        validation::declared_place_type_raw(&checked.typed, owner, Some(source), assignment.target)
            .ok_or(LoweringError::Unsupported(
                "array replacement lost its declared destination",
            ))?;
    let leaves = array_literal_leaves(checked, literal, assignment.value, declared).ok_or(
        LoweringError::Unsupported("array replacement is not an exact fixed array literal"),
    )?;
    let (producers, windows) = roster.split_at(leaves.len().min(roster.len()));
    let mut produced = Vec::with_capacity(leaves.len());
    for (producer, leaf) in producers.iter().zip(&leaves) {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            value,
            discard_result_on_return: false,
            operand_source: None,
            ..
        } = producer
        else {
            return unsupported("array replacement has no element producer");
        };
        if *value != leaf.value
            || result.statement_index as usize != ordinal
            || result.type_identity
                != checked
                    .normalized_type_identity(leaf.type_reference)
                    .as_str()
        {
            return unsupported("array replacement reordered its elements");
        }
        produced.push(result);
    }
    if produced.len() != leaves.len() {
        return unsupported("array replacement dropped an element");
    }
    let target = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        owner.symbol,
        state,
        Some(ordinal),
        assignment.target,
    )?;
    let mut windows = windows.iter().peekable();
    for (leaf, produced) in leaves.iter().zip(produced) {
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
            return unsupported("array replacement lost an element store");
        };
        let mut expected_path = target.path.clone();
        expected_path.extend(
            leaf.indices
                .iter()
                .map(|index| checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(*index)),
        );
        let root = destination
            .source_parameter_index()
            .and_then(|index| parameters.get(index as usize))
            .and_then(|parameter| {
                checked
                    .state_parameters(source)
                    .get(parameter.position as usize)
            })
            .ok_or(LoweringError::Unsupported(
                "array replacement has no authored parameter root",
            ))?;
        if destination.access != checked_trees::CheckedStructuralAccess::Owned
            || destination.type_identity != produced.type_identity
            || target.root != root.symbol
            || destination.path != expected_path
            || moved.is_some_and(|(moved, place)| {
                place != destination
                    || moved.type_identity != produced.type_identity
                    || moved.multiplicity != produced.multiplicity
                    || moved.statement_index != produced.statement_index
            })
        {
            return unsupported("array replacement window names another element");
        }
        if value.source
            != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: produced.binding_ordinal,
            })
            || !value.path.is_empty()
            || value.access != checked_trees::CheckedStructuralAccess::Owned
            || value.type_identity != produced.type_identity
        {
            return unsupported("array replacement stores another element");
        }
        // The displaced value is disposed exactly as the checked plan retires
        // it: an affine value dies whole on the statement's continuation; a
        // copy value has no disposal obligation and no continuation.
        match moved.map(|(moved, _)| moved) {
            Some(moved) if moved.multiplicity == Multiplicity::Affine => {
                let displaced = checked_trees::CheckedUnitPartialAffineDiscardPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: moved.binding_ordinal,
                        },
                    path: Vec::new(),
                    type_identity: produced.type_identity.clone(),
                };
                let Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards,
                }) = windows.next()
                else {
                    return unsupported("array replacement lost a displaced element's disposal");
                };
                if coordinate.statement_index != produced.statement_index
                    || coordinate.call_ordinal != 0
                    || affine_discards.as_slice() != std::slice::from_ref(&displaced)
                {
                    return unsupported("array replacement lost a displaced element's disposal");
                }
            }
            Some(moved) if moved.multiplicity != Multiplicity::Unrestricted => {
                return unsupported("array replacement displaces a linear element");
            }
            _ => {}
        }
    }
    if windows.next().is_some() {
        return unsupported("array replacement added an operation");
    }
    Ok(())
}
