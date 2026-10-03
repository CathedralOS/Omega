//! Rejoin local establishment and the authored per-edge ownership partition.
//!
//! StateExit records the lexical affine remainder even when a selected edge
//! transfers that local. It is not an instruction to drop on every successor.
//! Reconstruct the transfer roster from source, check its exact permission rows,
//! then subtract the selected edge's transfers. A transfer is a whole local
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
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{StatementNode, TransitionTargetNode};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
};

#[cfg(test)]
mod tests;

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
    checked: &CheckedTrees,
    statements: &[StatementNode],
    state: &CheckedComposedUnitControlStatePlan,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> bool {
    matches!(
        &state.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. }
            if matches!(
                subject.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } if binding_ordinal == result.binding_ordinal
            )
    ) && crate::expression_preparation::source_custody::dispatch_subjects::evaluated_case_subject(
        checked,
        statements,
        result.statement_index as usize,
    )
    .is_some()
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
    source: PermissionEventSource,
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
                // Boundary settlement retains its own completion receipts.
                if call.is_boundary() {
                    continue;
                }
                let authored = crate::emission::call_source_custody::authored::locate_source(
                    checked,
                    state.state,
                    call.coordinate,
                )?;
                let source = PermissionEventSource::Call {
                    statement_index: call.coordinate.statement_index as usize,
                    call_ordinal: call.coordinate.call_ordinal as usize,
                    target_symbol: authored.source_target,
                };
                inputs.extend(call.structural_arguments.iter().map(|argument| BodyInput {
                    operation_index,
                    source,
                    argument,
                    projected: false,
                }));
            }
            let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } =
                operation
            else {
                continue;
            };
            let source = PermissionEventSource::Statement {
                statement_index: result.statement_index as usize,
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
                            source,
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
/// Named affine locals also retire at ordinary calls when
/// their unique whole move rejoins exact establishment and transfer evidence.
/// Shared call admission checks each authored operand before emission.
fn retired_in_body(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
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
            CheckedUnitEffectOperationPlan::CallContinuationCleanup {
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
            loan,
            ..
        } = operation
            && !super::super::emission::reference_release_is_deferred(
                checked,
                machine,
                state.state,
                *loan,
            )?
        {
            retired.push(*binding_ordinal);
        }
    }
    // Calls and value construction both transfer whole operands. A result's
    // discard flag alone cannot prove retirement: rejoin its one consuming
    // operand and exact establishment/transfer receipts. Each consumer also
    // independently validates its authored operands before emission.
    let inputs = body_inputs(checked, state)?;
    for (producer_index, operation) in state.operations.iter().enumerate() {
        let Some(result) = result(operation) else {
            continue;
        };
        if result.multiplicity != Multiplicity::Affine || retired.contains(&result.binding_ordinal)
        {
            continue;
        }
        let statements = checked.statement_table.statements(source.statement_nodes);
        let Some(StatementNode::LocalData(local)) = statements.get(result.statement_index as usize)
        else {
            continue;
        };
        let uses = inputs.iter().filter(|input| {
            input.argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                || matches!(input.argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                        if symbol == local.symbol)
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
        // A stored-loan record is an owned carrier too: its affine whole moves
        // whole, and the leaves' borrow custody lives in the loan ledger the
        // construction replay already joined. Retiring it demands the same
        // exact captured-loan evidence the establishment check requires --
        // admission is the record loan join, not the plain-contents shape.
        let stored_loan_record = validation::reference_result_custody::local_owes_record_loans(
            &checked.typed,
            local.type_reference,
        ) && validation::reference_result_custody::local_record_loans(
            &checked.typed,
            &checked.facts,
            machine,
            source,
            result.statement_index,
        )
        .is_some();
        if checked.type_multiplicity(local.type_reference) != Multiplicity::Affine
            || checked
                .normalized_type_identity(local.type_reference)
                .as_str()
                != result.type_identity
            || !(stored_loan_record
                || validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    local.type_reference,
                ))
        {
            return unsupported("Unit graph operand retirement changed its local custody");
        }
        let establishment = PermissionEventSource::Statement {
            statement_index: result.statement_index as usize,
        };
        let transfer = consumer.source;
        // A local move preserves its value's original provenance. Reuse the
        // construction replay (including its borrowed-loan checks), then
        // require this precise consuming operand as the sole final disposition.
        crate::expression_preparation::source_custody::structural::validate_local_ownership(
            checked,
            machine,
            state.state,
            result.statement_index,
            local.symbol,
            local.type_reference,
            result.multiplicity,
            false,
        )?;
        let mut establishes = 0;
        let mut transfers = 0;
        for (_, event) in checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine
                    && event.state_symbol == state.state
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
                    && event.access == PermissionAccess::Owned
            })
        {
            if event.kind == PermissionEventKind::Establish && event.source == establishment {
                establishes += 1;
            } else if matches!(
                event.kind,
                PermissionEventKind::Transfer | PermissionEventKind::Consume
            ) && event.source == transfer
            {
                transfers += 1;
            } else {
                return unsupported("Unit graph moved local retains another disposition");
            }
        }
        if establishes != 1 || transfers != 1 {
            return unsupported("Unit graph operand retirement lost its exact ownership receipts");
        }
        retired.push(result.binding_ordinal);
    }
    Ok(retired)
}

/// Validate the complete disposal roster, not the producer's representation.
///
/// Construction replay and call-source replay independently check result type,
/// multiplicity and exact source before publication. Repeating that judgment
/// from the local's syntax confuses a shared alias of existing storage with a
/// materialized reference result: both spell `&T`, but only the latter creates
/// a carrier to retire. This layer checks the resulting ownership receipts;
/// the loan ledger separately owns reference lifetimes and restored access.
pub(super) fn validate_disposition(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    local: &checked_trees::statement::TableLocalData,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let events = || {
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == source.symbol
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
                    // Loan access may name the same home without establishing
                    // or consuming its owned value. Borrow custody validates
                    // those records; this roster rejoins owned dispositions.
                    && event.access == PermissionAccess::Owned
            })
    };
    // Plain copy payloads retain producer dominance, not affine receipts. The
    // ordinary body rejoin establishes that producer before any edge use.
    if result.multiplicity == Multiplicity::Unrestricted {
        return if events().next().is_none() {
            Ok(())
        } else {
            unsupported("Unit graph copy local acquired ownership debt")
        };
    }
    // A view local's exclusivity is the loan ledger's -- its establish,
    // transfer and weakening are the reference custody ops, not ownership
    // events, so the ownership roster is exactly empty. A shared record loan
    // carries the same accounting read-side.
    if validation::reference_shell(&checked.typed, local.type_reference).is_some() {
        return if events().next().is_none() {
            Ok(())
        } else {
            unsupported("Unit graph view local acquired ownership debt")
        };
    }
    let role = selection_role(
        checked,
        machine,
        source.symbol,
        local.symbol,
        result.statement_index,
    );
    let mut expected = Vec::new();
    if role != Some(SelectionRole::Destination) {
        expected.push((
            PermissionEventKind::Establish,
            PermissionEventSource::Statement {
                statement_index: result.statement_index as usize,
            },
        ));
    }
    match role {
        // The receipt's arm transfers establish the destination; its residual
        // parameters, not the dead source place, carry each source's death.
        Some(SelectionRole::Destination) => expected.push((
            PermissionEventKind::AffineDrop,
            PermissionEventSource::StateExit,
        )),
        Some(SelectionRole::Source) => {}
        None if result.multiplicity == Multiplicity::Affine => expected.push((
            PermissionEventKind::AffineDrop,
            PermissionEventSource::StateExit,
        )),
        None => {
            return unsupported("Unit graph local requires an explicit linear disposition");
        }
    }
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = checked.statement_table.transition_target(transition.target)
        else {
            continue;
        };
        for expression in checked.statement_table.expression_handles(*arguments) {
            if matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(name)
                if name.symbol == local.symbol && name.head_symbol == local.symbol
                    && checked.expression_table.name_path_members(name.members).len() == 1)
            {
                expected.push((
                    PermissionEventKind::Transfer,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal: 0,
                        target_symbol: path.symbol,
                    },
                ));
            }
        }
        // An operand call of this edge that takes the local whole moves it
        // at the call's own occurrence, after the edge is selected.
        let operand_moves = validation::transition_operand_moves(
            &checked.typed,
            &checked.facts,
            machine,
            source,
            statement_index,
            local.symbol,
        )
        .ok_or(LoweringError::Unsupported(
            "Unit graph local operand call lost its authored occurrence",
        ))?;
        expected.extend(
            operand_moves
                .into_iter()
                .map(|source| (PermissionEventKind::Transfer, source)),
        );
    }
    // A local's initializer can conserve an earlier value's origin. Reuse
    // the ordinary construction replay rather than manufacturing provenance
    // from the destination declaration's location.
    let provenance =
        crate::expression_preparation::source_custody::structural::local_value_provenance(
            checked,
            machine,
            source.symbol,
            result.statement_index as usize,
        )?;
    if events().count() != expected.len() {
        return unsupported("Unit graph local ownership roster drifted");
    }
    for (kind, source) in expected {
        let mut matching = events().filter(|event| event.kind == kind && event.source == source);
        let event = matching.next().ok_or(LoweringError::Unsupported(
            "Unit graph local ownership evidence missing",
        ))?;
        if matching.next().is_some()
            || event.multiplicity != result.multiplicity
            || event.access != PermissionAccess::Owned
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
            || !event.segments.is_empty()
        {
            return unsupported("Unit graph local ownership origin or disposition drifted");
        }
    }
    Ok(())
}

/// Every local exit disposition must belong to a retained result. Parameter
/// dispositions are checked separately against their entry custody.
pub(super) fn validate_disposition_roster(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let cleanup_discards = retired_in_body(checked, machine, source, state)?;
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
    {
        if checked
            .state_parameters(source)
            .iter()
            .any(|parameter| event.root == facts::PlaceRoot::Symbol(parameter.symbol))
        {
            continue;
        }
        // Argument constructions share the enclosing local's statement, but
        // their custody already moved into the call. Rejoin the same live
        // result roster used by local_discards and edge disposal below.
        let matching = state.operations.iter().filter_map(result).filter(|result| {
            !cleanup_discards.contains(&result.binding_ordinal) &&
            matches!(statements.get(result.statement_index as usize),
                Some(StatementNode::LocalData(local)) if event.root == facts::PlaceRoot::Symbol(local.symbol))
        }).count();
        if matching != 1 {
            return unsupported("Unit graph edge leaves an unaccounted local disposition");
        }
    }
    Ok(())
}

pub(super) fn local_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: Option<&CheckedStructuralControlSuccessorPlan>,
) -> Result<Vec<u32>, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let cleanup_discards = retired_in_body(checked, machine, source, state)?;
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
        if !computed_case_subject(checked, statements, state, result) {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit graph edge result has no authored local");
            };
            if edge.is_some_and(|edge| result.statement_index >= edge.statement_ordinal) {
                return unsupported("Unit graph edge precedes local establishment");
            }
            validate_disposition(checked, machine, source, local, result)?;
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

/// How many of `edge`'s operand calls move `result` out of its home.
///
/// The plan's count (`CheckedStructuralControlSuccessorPlan::operand_result_moves`)
/// must equal the authored occurrences that pass the result's local whole,
/// each of which `validate` already rejoined to its one ledger Transfer row.
/// A copyable result's operand reads are copies and move nothing. A plan
/// that names a move the source does not perform, or omits one it does,
/// would dispose of a moved value or leak a live one, so it declines.
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
    let planned = edge.operand_result_moves(result.binding_ordinal);
    let authored = match local {
        Some(local) => validation::transition_operand_moves(
            &checked.typed,
            &checked.facts,
            machine,
            source,
            edge.statement_ordinal as usize,
            local.symbol,
        )
        .ok_or(LoweringError::Unsupported(
            "Unit graph local operand call lost its authored occurrence",
        ))?
        .len(),
        None => 0,
    };
    if planned != authored {
        return unsupported("Unit graph operand call moves drifted from their authored local");
    }
    Ok(planned)
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
    let cleanup_discards = retired_in_body(checked, machine, source, state)?;
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
        validate_disposition(checked, machine, source, local, result)?;
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
