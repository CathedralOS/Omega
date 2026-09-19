//! Rejoin local establishment and the authored per-edge ownership partition.
//!
//! StateExit records the lexical affine remainder even when a selected edge
//! transfers that local. It is not an instruction to drop on every successor.
//! Reconstruct the transfer roster from source, check its exact permission rows,
//! then subtract the selected edge's transfers. The resulting reverse-ordered
//! local roots use Terminal's existing edge disposal, after argument evaluation.
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
    PermissionProvenance,
};

pub(super) fn result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&checked_trees::CheckedUnitStructuralResultBindingPlan> {
    match operation {
        CheckedUnitEffectOperationPlan::StructuralCall {
            result,
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            result,
            discard_result_on_return: false,
            ..
        }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return: false,
            ..
        } => Some(result),
        _ => None,
    }
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

/// Validate the complete roster, not a requirement to transfer on every branch.
pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    local: &checked_trees::statement::TableLocalData,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    if checked.type_multiplicity(local.type_reference) != result.multiplicity
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("Unit graph local disposition type drifted");
    }
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
    }
    let provenance = match role {
        // The destination keeps its receipt's Unknown provenance; its sources
        // keep the ordinary establishment provenance of their own statements.
        Some(SelectionRole::Destination) => PermissionProvenance::Unknown,
        _ => PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: source.symbol,
            source: PermissionEventSource::Statement {
                statement_index: result.statement_index as usize,
            },
        },
    };
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
        let matching = state.operations.iter().filter_map(result).filter(|result| {
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
    // A discarded structural result pairs with its call's cleanup continuation
    // rather than an authored local; that continuation already proves disposal.
    let cleanup_discards =
        state
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    affine_discards, ..
                } => {
                    Some(affine_discards.iter().filter_map(|discard| {
                        match discard.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => Some(binding_ordinal),
                    _ => None,
                }
                    }))
                }
                _ => None,
            })
            .flatten()
            .collect::<Vec<u32>>();
    let mut discards = Vec::new();
    for result in state.operations.iter().rev().filter_map(result) {
        if cleanup_discards.contains(&result.binding_ordinal) {
            continue;
        }
        let Some(StatementNode::LocalData(local)) = statements.get(result.statement_index as usize)
        else {
            return unsupported("Unit graph edge result has no authored local");
        };
        if edge.is_some_and(|edge| result.statement_index >= edge.statement_ordinal) {
            return unsupported("Unit graph edge precedes local establishment");
        }
        validate(checked, machine, source, local, result)?;
        if selection_role(
            checked,
            machine,
            source.symbol,
            local.symbol,
            result.statement_index,
        ) == Some(SelectionRole::Source)
        {
            // The source's own place already died at the selection join; its
            // residual parameter is disposed by the caller's splice roster.
            continue;
        }
        let transferred = edge.into_iter().flat_map(|edge| &edge.transfers).filter(|transfer| matches!(transfer.source,
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal }
                if binding_ordinal == result.binding_ordinal)).count();
        if transferred > 1 {
            return unsupported("Unit graph edge duplicates local ownership");
        }
        if transferred == 0 && result.multiplicity == Multiplicity::Affine {
            discards.push(result.binding_ordinal);
        }
    }
    Ok(discards)
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
    // A discarded structural result pairs with its call's cleanup continuation
    // rather than an authored local; that continuation already proves disposal.
    let cleanup_discards =
        state
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    affine_discards, ..
                } => {
                    Some(affine_discards.iter().filter_map(|discard| {
                        match discard.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => Some(binding_ordinal),
                    _ => None,
                }
                    }))
                }
                _ => None,
            })
            .flatten()
            .collect::<Vec<u32>>();
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
        let place = case_emission::result(state, result.binding_ordinal, operations)?.place;
        let role = selection_role(
            checked,
            machine,
            source.symbol,
            local.symbol,
            result.statement_index,
        );
        validate(checked, machine, source, local, result)?;
        let transferred = edge
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
