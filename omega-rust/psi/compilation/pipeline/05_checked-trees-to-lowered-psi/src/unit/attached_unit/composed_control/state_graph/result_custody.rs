//! Rejoin local establishment and the authored per-edge ownership partition.
//!
//! StateExit records the selected affine remainder at each exact normal exit.
//! Distinct outgoing edges retain distinct evaluation coordinates.
//! Consume the checked transfer roster and subtract the selected edge's transfers. A transfer is a whole local
//! passed to the target state or, after selection, into one of the edge's
//! own operand calls (`next(consume(local))`); either moves the local on that
//! edge alone, and every sibling edge still disposes of it. The resulting
//! reverse-ordered local roots use Terminal's existing edge disposal, after
//! argument evaluation.
use super::super::super::{CheckedUnitEffectOperationPlan, Multiplicity, PlaceId, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlStatePlan, CheckedStructuralControlSuccessorPlan, case_emission,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::statement::StatementNode;
use language_semantics::PermissionEventSource;

/// A call or constructed-value result that no automatic return discard
/// retires: the selected edge owns its disposal.
pub(super) fn result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    operation
        .discardable_structural_result()
        .and_then(|(result, discard_result_on_return)| {
            (!discard_result_on_return).then_some(result)
        })
}

/// Whether `result` is the computed subject of this state's closed-case
/// dispatch: the anonymous result the dispatch's first arm mints and every
/// case edge consumes. It names no local and has no permission rows.
fn computed_case_subject(
    state: &CheckedComposedUnitControlStatePlan,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> bool {
    matches!(
        &state.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum {
            subject,
            subject_kind: checked_trees::CheckedClosedSumSubjectKind::Computed,
            ..
        } if matches!(
            subject.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } if binding_ordinal == result.binding_ordinal
        )
    )
}

/// Which role an authored local plays in an owned-selection receipt, if any.
/// The destination's establishment and every source's residual death share the
/// receipt's StateExit authority rather than ordinary Establish rows; locals
/// outside any receipt keep their exact statement evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionRole {
    Destination,
    Source,
}

pub(super) fn selection_role(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    local: symbols::SymbolHandle,
    statement: u32,
) -> Option<SelectionRole> {
    checked
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .find_map(|(_, receipt)| {
            if receipt.machine != machine
                || receipt.state != state
                || receipt.death != PermissionEventSource::StateExit
            {
                return None;
            }
            if receipt.destination == local && receipt.statement_ordinal == statement {
                return Some(SelectionRole::Destination);
            }
            checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| source.symbol == local && source.statement_ordinal == statement)
                .then_some(SelectionRole::Source)
        })
}

/// One structural operand use at its enclosing operation occurrence. Projection
/// retains the root for residual disposal rather than consuming it whole.
struct BodyInput<'input> {
    operation_index: usize,
    argument: &'input checked_trees::CheckedUnitStructuralArgumentPlan,
    projected: bool,
}

fn body_inputs<'input>(
    checked: &'input CheckedTrees,
    state: &'input CheckedComposedUnitControlStatePlan,
) -> Result<Vec<BodyInput<'input>>, LoweringError> {
    use checked_trees::{CheckedStructuralRecordFieldValue, CheckedStructuralValueKind};
    let plans = &checked.facts.values.structural_values;
    let mut inputs = Vec::new();
    for (operation_index, owner) in state.operations.iter().enumerate() {
        for operation in owner.with_value_calls() {
            if let Some(call) = operation.call() {
                // Boundary and ordinary calls retire owned operands by the
                // same authored transfer. Boundary completion receipts govern
                // returned custody, not ownership of the caller's inputs.
                inputs.extend(call.structural_arguments.iter().map(|argument| BodyInput {
                    operation_index,
                    argument,
                    projected: false,
                }));
            }
            let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation
            else {
                continue;
            };
            let mut pending = vec![(*value, false)];
            let mut visited = Vec::new();
            while let Some((value, projected)) = pending.pop() {
                if !plans.nodes.is_valid(value) || visited.contains(&value) {
                    return unsupported(
                        "Unit graph operand inventory has stale or repeated value custody",
                    );
                }
                visited.push(value);
                match &plans.nodes.get(value).kind {
                    CheckedStructuralValueKind::Place(argument)
                    | CheckedStructuralValueKind::Reference { source: argument }
                    | CheckedStructuralValueKind::BorrowedSliceView {
                        source: argument, ..
                    }
                    | CheckedStructuralValueKind::MutableSliceView {
                        source: argument, ..
                    }
                    | CheckedStructuralValueKind::ScalarCasePlace {
                        source: argument, ..
                    }
                    | CheckedStructuralValueKind::CopiedStructuralPlace {
                        source: argument, ..
                    } => {
                        inputs.push(BodyInput {
                            operation_index,
                            argument,
                            projected,
                        });
                    }
                    CheckedStructuralValueKind::Record { fields, .. }
                    | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                        for field in
                            plans
                                .record_fields
                                .span(*fields)
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph operand fields are stale",
                                ))?
                        {
                            if let CheckedStructuralRecordFieldValue::Structural(child) =
                                field.value
                            {
                                pending.push((child, projected));
                            }
                        }
                    }
                    CheckedStructuralValueKind::FixedArray { elements } => {
                        pending.extend(elements.iter().map(|child| (*child, projected)));
                    }
                    CheckedStructuralValueKind::Projection { source, .. } => {
                        pending.push((*source, true))
                    }
                    // Selected source retirement is governed by its arm-transfer
                    // receipt, not an unconditional use of every alternative.
                    CheckedStructuralValueKind::Dispatch { .. }
                    | CheckedStructuralValueKind::Case(_)
                    | CheckedStructuralValueKind::Call { .. }
                    | CheckedStructuralValueKind::ByteLiteralView { .. }
                    | CheckedStructuralValueKind::ViewElementCopy { .. }
                    | CheckedStructuralValueKind::ScalarArray { .. }
                    | CheckedStructuralValueKind::ZeroedScalarArray { .. }
                    | CheckedStructuralValueKind::ZeroedStructural { .. } => {}
                }
            }
        }
    }
    Ok(inputs)
}

/// Structural results whose custody ends inside the body rather than at an
/// exit: a discarded result pairs with its call's cleanup continuation, a call
/// result stored whole into a borrowed field's opened window moves into that
/// field, and a construction authored as a call argument moves whole into the
/// call that reads it. An anonymous unrestricted byte result copied into its
/// assignment's destination or passed whole into its enclosing call has no
/// edge disposal or authored local. The call consumer independently rejoins
/// each occurrence, result contract and evaluation order before emission.
/// Named owned locals also retire at calls when
/// their unique whole move rejoins exact establishment and transfer evidence.
/// Shared call admission checks each authored operand before emission.
fn retired_in_body(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Vec<u32>, LoweringError> {
    let completed_operands = state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                operand_source:
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. }),
                ..
            } => Some(result.binding_ordinal),
            CheckedUnitEffectOperationPlan::Call(call)
                if !call.is_boundary() && call.coordinate.call_ordinal != 0 =>
            {
                call.structural_result()
                    .filter(|result| result.multiplicity == Multiplicity::Unrestricted)
                    .map(|result| result.binding_ordinal)
            }
            _ => None,
        })
        .filter(|ordinal| {
            state.operations.iter().any(|operation| {
                operation
                    .call_structural_arguments()
                    .iter()
                    .any(|argument| {
                        argument.source_structural_result_binding_ordinal() == Some(*ordinal)
                            && argument.access == checked_trees::CheckedStructuralAccess::Owned
                            && argument.path.is_empty()
                    })
            })
        });
    let mut retired = state
        .operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                affine_discards, ..
            } => affine_discards
                .iter()
                .filter_map(|discard| match discard.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => Some(binding_ordinal),
                    _ => None,
                })
                .collect(),
            CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } => value
                .source_structural_result_binding_ordinal()
                .into_iter()
                .collect(),
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => store
                .value
                .completed_call_binding_ordinal()
                .filter(|binding_ordinal| {
                    state.operations.iter().filter_map(result).any(|result| {
                        result.binding_ordinal == *binding_ordinal
                            && result.statement_index == store.statement_index
                            && result.multiplicity == Multiplicity::Unrestricted
                    })
                })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        })
        .chain(completed_operands)
        .collect::<Vec<_>>();
    // A body release already consumed this exact carrier. Only loans whose
    // last use belongs to a transition remain for the selected edge to drop.
    for operation in &state.operations {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            path,
            defer_to_exit: false,
            ..
        } = operation
            && path.is_empty()
        {
            retired.push(*binding_ordinal);
        }
    }
    // Calls and value construction both transfer whole operands. Use the
    // checked producer/consumer order to exclude transferred homes from exit
    // cleanup. Source ownership admission already accounted for the move.
    let inputs = body_inputs(checked, state)?;
    for (producer_index, operation) in state.operations.iter().enumerate() {
        let Some(result) = result(operation) else {
            continue;
        };
        if result.multiplicity == Multiplicity::Unrestricted
            || retired.contains(&result.binding_ordinal)
        {
            continue;
        }
        let statements = checked.statement_table.statements(source.statement_nodes);
        // Nested calls mint anonymous operands even inside a local initializer.
        // Their call occurrence owns the result; the enclosing statement's
        // local symbol identifies a different value produced by its outer call.
        let anonymous_call = operation.call().filter(|call| {
            call.coordinate.call_ordinal != 0
                && matches!(
                    call.source_site().flatten(),
                    Some(checked_trees::NominalMachineUseSite::Expression(_))
                )
        });
        let local = if anonymous_call.is_some() {
            None
        } else if let Some(StatementNode::LocalData(local)) =
            statements.get(result.statement_index as usize)
        {
            Some(local)
        } else {
            continue;
        };
        let uses = inputs.iter().filter(|input| {
            input.argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                || local.is_some_and(|local| matches!(input.argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                        if symbol == local.symbol))
        }).collect::<Vec<_>>();
        let mut moves = uses.iter().filter(|input| {
            input.argument.access == checked_trees::CheckedStructuralAccess::Owned
                && input.argument.path.is_empty()
                && !input.projected
        });
        let Some(consumer) = moves.next().copied() else {
            continue;
        };
        if moves.next().is_some()
            || consumer.operation_index <= producer_index
            || consumer.argument.type_identity != result.type_identity
            || uses.iter().any(|input| {
                input.operation_index <= producer_index
                    || input.operation_index > consumer.operation_index
                    || (input.argument.access == checked_trees::CheckedStructuralAccess::Owned
                        && (!input.argument.path.is_empty() || input.projected))
            })
        {
            return unsupported("Unit graph operand retirement duplicates or precedes its owner");
        }
        if local.is_none() {
            // The inventory includes calls retained inside selected values.
            // Only the direct body operation proves this anonymous operand
            // is consumed on every path before control reaches an exit.
            let direct_consumer = state.operations[consumer.operation_index]
                .call()
                .is_some_and(|call| {
                    call.structural_arguments
                        .iter()
                        .any(|argument| std::ptr::eq(argument, consumer.argument))
                });
            if !direct_consumer {
                return unsupported("Unit graph anonymous operand has no unconditional consumer");
            }
            let _ = anonymous_call.ok_or(LoweringError::Unsupported(
                "Unit graph anonymous operand has no authored producer",
            ))?;
            retired.push(result.binding_ordinal);
            continue;
        }
        retired.push(result.binding_ordinal);
    }
    Ok(retired)
}

/// Locals already disposed at a checked statement entry must not be disposed
/// again at return. Stage 04 owns the lifetime decision; lowering consumes its
/// result ordinals directly instead of reconstructing source ownership rows.
pub(super) fn statement_discards(state: &CheckedComposedUnitControlStatePlan) -> Vec<u32> {
    state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. },
                affine_discards,
            } => Some(affine_discards),
            _ => None,
        })
        .flatten()
        .filter_map(|discard| match discard.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } => Some(binding_ordinal),
            _ => None,
        })
        .collect()
}

pub(super) fn local_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: Option<&CheckedStructuralControlSuccessorPlan>,
) -> Result<Vec<u32>, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let cleanup_discards = retired_in_body(checked, source, state)?;
    let mut discards = Vec::new();
    for result in state.operations.iter().rev().filter_map(result) {
        if cleanup_discards.contains(&result.binding_ordinal) {
            continue;
        }
        // An ordinary structural return transfers its exact result; other
        // local owners still need the same cleanup as a Unit completion.
        if edge.is_none()
            && matches!(&state.terminator,
                checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned }
                    if matches!(returned.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
                            if binding_ordinal == result.binding_ordinal))
        {
            continue;
        }
        // The case edges that consume a computed dispatch subject leave from
        // its own statement; the edge emitter removes it from their roster.
        let mut authored_local = None;
        if !computed_case_subject(state, result) {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit graph edge result has no authored local");
            };
            if edge.is_some_and(|edge| result.statement_index >= edge.statement_ordinal) {
                return unsupported("Unit graph edge precedes local establishment");
            }
            if selection_role(
                checked,
                machine,
                source.symbol,
                local.symbol,
                result.statement_index,
            ) == Some(SelectionRole::Source)
            {
                // The source's own place already died at the selection join;
                // its residual parameter is disposed by the caller's splice
                // roster.
                continue;
            }
            authored_local = Some(local);
        }
        let operand_call_moves = match edge {
            Some(edge) => operand_moves(checked, machine, source, authored_local, result, edge)?,
            None => 0,
        };
        let transferred = operand_call_moves
            + edge
                .into_iter()
                .flat_map(|edge| &edge.transfers)
                .filter(|transfer| {
                    transfer_holds_result(checked, source, state, transfer, result.binding_ordinal)
                })
                .count();
        if transferred > 1 {
            return unsupported("Unit graph edge duplicates local ownership");
        }
        if transferred == 0 && result.multiplicity == Multiplicity::Affine {
            discards.push(result.binding_ordinal);
        }
    }
    Ok(discards)
}

/// The source-state result whose home the view `binding_ordinal` lends. A
/// `let` view (`room.exits.as_mut_slice()`) is a loan of the local it was
/// formed from, so an edge carrying the view must keep that local's home live
/// with the loan, exactly as a projected loan keeps its backing local. `None`
/// for a non-view result or a view of storage the state does not own (a
/// parameter outlives every state of the activation).
pub(super) fn view_backing(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
) -> Option<u32> {
    let value =
        state
            .operations
            .iter()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result, value, ..
                } if result.binding_ordinal == binding_ordinal => Some(*value),
                _ => None,
            })?;
    let nodes = &checked.facts.values.structural_values.nodes;
    let lent = match &nodes.get(nodes.is_valid(value).then_some(value)?).kind {
        checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
        | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => source,
        _ => return None,
    };
    match lent.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => Some(binding_ordinal),
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            let statements = checked.statement_table.statements(source.statement_nodes);
            let mut homes = state.operations.iter().filter_map(result).filter(|home| {
                matches!(
                    statements.get(home.statement_index as usize),
                    Some(StatementNode::LocalData(local)) if local.symbol == symbol
                )
            });
            let home = homes.next()?;
            homes.next().is_none().then_some(home.binding_ordinal)
        }
        _ => None,
    }
}

/// Whether `transfer` takes the source state's result `binding_ordinal` off
/// its edge's cleanup roster: a whole transfer to the target, or a loan --
/// projected, or a carried view -- whose backing local stays live through the
/// edge and dies with the loan, never on this edge itself.
fn transfer_holds_result(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transfer: &checked_trees::CheckedStructuralControlTransferPlan,
    binding_ordinal: u32,
) -> bool {
    match &transfer.source {
        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
            binding_ordinal: transferred,
        } => {
            *transferred == binding_ordinal
                || view_backing(checked, source, state, *transferred) == Some(binding_ordinal)
        }
        checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            subject,
            ..
        } => subject.source_structural_result_binding_ordinal() == Some(binding_ordinal),
        _ => false,
    }
}

/// How many of `edge`'s operand calls move `result` out of its home: the
/// plan's count (`CheckedStructuralControlSuccessorPlan::operand_result_moves`)
/// of the authored occurrences that pass the result's local whole. A copyable
/// result's operand reads are copies and move nothing.
fn operand_moves(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    local: Option<&checked_trees::statement::TableLocalData>,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<usize, LoweringError> {
    if result.multiplicity == Multiplicity::Unrestricted {
        return Ok(0);
    }
    if let Some(local) = local
        && validation::transition_operand_moves(
            &checked.typed,
            &checked.facts,
            machine,
            source,
            edge.statement_ordinal as usize,
            local.symbol,
        )
        .is_none()
    {
        return unsupported("Unit graph local operand call lost its authored occurrence");
    }
    Ok(edge.operand_result_moves(result.binding_ordinal))
}

/// Rejoin the owned-selection frontier to one ordinary successor's cleanup
/// edge. The roster keeps the same reverse-establishment order as the return
/// splice: source rows stay unflagged so `selection_return_discards` can
/// substitute each residual parameter at its own positional slot, and every
/// other live result flags whether this edge still owns it.
pub(super) fn selection_edge_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
    operations: &OperationBuffer,
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
) -> Result<Vec<PlaceId>, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let cleanup_discards = retired_in_body(checked, source, state)?;
    let mut roots = Vec::new();
    for result in state.operations.iter().rev().filter_map(result) {
        if cleanup_discards.contains(&result.binding_ordinal) {
            continue;
        }
        let Some(StatementNode::LocalData(local)) = statements.get(result.statement_index as usize)
        else {
            return unsupported("Unit graph edge result has no authored local");
        };
        if result.statement_index >= edge.statement_ordinal {
            return unsupported("Unit graph edge precedes local establishment");
        }
        let place = case_emission::result(state, result.binding_ordinal, operations)?.place();
        let role = selection_role(
            checked,
            machine,
            source.symbol,
            local.symbol,
            result.statement_index,
        );
        let transferred = operand_moves(checked, machine, source, Some(local), result, edge)?
            + edge
                .transfers
                .iter()
                .filter(|transfer| {
                    matches!(transfer.source,
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                            if binding_ordinal == result.binding_ordinal)
                })
                .count();
        if transferred > 1 || (role == Some(SelectionRole::Source) && transferred != 0) {
            return unsupported("Unit graph edge duplicates local ownership");
        }
        let dies = role != Some(SelectionRole::Source)
            && result.multiplicity == Multiplicity::Affine
            && transferred == 0;
        roots.push((place, dies));
    }
    // Selected parameter sources ride the same roster after every result row:
    // the receipt keeps them last in descending authored-position order, and
    // the residual join parameters replace each row at its own slot.
    let mut parameter_sources = Vec::new();
    for cleanup in &evaluation.selection_cleanups {
        for source in &cleanup.sources {
            if let Some((position, _)) = evaluation
                .structural_parameters
                .iter()
                .find(|(_, declaration)| declaration.place == *source)
            {
                parameter_sources.push((*position, *source));
            }
        }
    }
    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    parameter_sources.dedup_by_key(|(_, place)| *place);
    roots.extend(
        parameter_sources
            .into_iter()
            .map(|(_, place)| (place, false)),
    );
    evaluation.selection_return_discards(roots)
}
