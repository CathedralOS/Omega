//! Rejoin a temporary owner's establishment, shared loan, and dying continuation.

use super::*;
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};

pub(super) fn validate(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    producer: checked_trees::CheckedUnitCallCoordinate,
    consumer: checked_trees::CheckedUnitCallCoordinate,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<(), LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::Call(call)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(consumer.statement_index as usize)
    else {
        return unsupported("anonymous shared result requires one authored Unit call");
    };
    if call.arguments.count() != 1
        || checked
            .statement_table
            .expression_handles(call.arguments)
            .len()
            != 1
    {
        return unsupported("anonymous shared result requires one authored argument");
    }
    let mut states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == caller.machine && state.state_symbol == caller.state
        });
    let (_, state) = states.next().ok_or(LoweringError::Unsupported(
        "anonymous shared result has no captured state",
    ))?;
    if states.next().is_some() {
        return unsupported("anonymous shared result has ambiguous captured states");
    }
    let calls =
        checked
            .facts
            .flow
            .control
            .calls
            .span(state.calls)
            .ok_or(LoweringError::Unsupported(
                "anonymous shared result has a stale captured call span",
            ))?;
    if calls
        .iter()
        .filter(|call| call.statement_index == consumer.statement_index as usize)
        .count()
        != 2
    {
        return unsupported("anonymous shared result has additional captured calls");
    }
    let call_source = |coordinate: checked_trees::CheckedUnitCallCoordinate| {
        let authored =
            crate::call_source_custody::authored::locate_source(checked, caller.state, coordinate)?;
        let mut matching = calls.iter().filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
        });
        let call = matching.next().ok_or(LoweringError::Unsupported(
            "anonymous shared result has no captured call",
        ))?;
        if matching.next().is_some() || call.target_symbol != authored.source_target {
            return unsupported("anonymous shared result call identity drifted");
        }
        Ok(PermissionEventSource::Call {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            target_symbol: call.target_symbol,
        })
    };
    let producer_source = call_source(producer)?;
    let consumer_source = call_source(consumer)?;
    let provenance = PermissionProvenance::Established {
        machine_symbol: caller.machine,
        state_symbol: caller.state,
        source: producer_source,
    };
    let mut events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == caller.machine
                && event.state_symbol == caller.state
                && event.root == facts::PlaceRoot::Expression(expression)
        });
    for (kind, source, access, multiplicity) in [
        (
            PermissionEventKind::Establish,
            producer_source,
            PermissionAccess::Owned,
            Multiplicity::Affine,
        ),
        (
            PermissionEventKind::Establish,
            consumer_source,
            PermissionAccess::Shared,
            Multiplicity::Unrestricted,
        ),
        (
            PermissionEventKind::AffineDrop,
            consumer_source,
            PermissionAccess::Owned,
            Multiplicity::Affine,
        ),
    ] {
        let (_, event) = events.next().ok_or(LoweringError::Unsupported(
            "anonymous shared result permission sequence is incomplete",
        ))?;
        if event.kind != kind
            || event.source != source
            || event.access != access
            || event.multiplicity != multiplicity
            || event.provenance != provenance
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.obligation_live
            || checked.facts.flow.ownership.segments.span(event.segments) != Some(&[][..])
        {
            return unsupported("anonymous shared result permission custody drifted");
        }
    }
    if events.next().is_some() {
        return unsupported("anonymous shared result permission sequence has extra events");
    }
    Ok(())
}

pub(crate) fn validate_cleanup(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation_index: usize,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards,
    } = &caller.operations[operation_index]
    else {
        return unsupported("call continuation cleanup entry is absent");
    };
    let Some(CheckedUnitEffectOperationPlan::CallUnit {
        coordinate: call,
        structural_arguments,
        ..
    }) = operation_index
        .checked_sub(1)
        .and_then(|previous| caller.operations.get(previous))
    else {
        return unsupported("call cleanup does not immediately follow its consumer");
    };
    let ([discard], [argument]) = (affine_discards.as_slice(), structural_arguments.as_slice())
    else {
        return unsupported("call cleanup requires an exact temporary argument list");
    };
    if call != coordinate
        || argument.source != discard.source
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || !argument.path.is_empty()
        || !discard.path.is_empty()
        || argument.type_identity != discard.type_identity
    {
        return unsupported("call cleanup substituted its consumer or result");
    }
    let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
        return unsupported("call cleanup requires an expression-owned result");
    };
    let mut producers = caller.operations[..operation_index]
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => {
                Some((*coordinate, result, *discard_result_on_return))
            }
            _ => None,
        });
    let (producer, result, discard_on_return) = producers.next().ok_or(
        LoweringError::Unsupported("call cleanup has no result producer"),
    )?;
    if producers.next().is_some()
        || discard_on_return
        || producer.call_ordinal == 0
        || producer.statement_index != coordinate.statement_index
        || result.type_identity != discard.type_identity
    {
        return unsupported("call cleanup has no unique continuing owner");
    }
    let source =
        crate::call_source_custody::authored::locate_source(checked, caller.state, producer)?;
    let Some(checked_trees::NominalMachineUseSite::Expression(expression)) = source.source_site
    else {
        return unsupported("call cleanup lost its expression-owned source");
    };
    validate(checked, caller, producer, *coordinate, expression)
}
