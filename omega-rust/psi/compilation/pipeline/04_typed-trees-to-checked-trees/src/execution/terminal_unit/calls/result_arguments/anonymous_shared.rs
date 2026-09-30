//! A shared call retains and then disposes the actual temporary owner.
use super::{
    CheckFacts, ExpressionNode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::MachineSupplyMode;
use crate::execution::terminal_unit::types::state_flow;

pub(super) fn validate(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<()> {
    let target = program.machines().iter().find(|target| {
        program
            .machine_states(target)
            .iter()
            .any(|target_state| target_state.symbol == consumer.target_symbol)
    })?;
    if target.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let flow = state_flow(facts, machine, state)?;
    let mut producers = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|call| {
            call.statement_index == consumer.statement_index
                && call.authored_expression == expression
        });
    let producer = producers.next()?;
    if producers.next().is_some() || producer.call_ordinal <= consumer.call_ordinal {
        return None;
    }
    let source = crate::semantic::calls::find_call_site(
        program,
        machine,
        state,
        consumer.statement_index,
        consumer.call_ordinal,
    )?;
    // The loan belongs to one exact operand, not to a one-argument source
    // idiom. Sibling producers, scalar results and surrounding computations
    // do not change this owner's establishment or its dying continuation.
    if crate::semantic::calls::call_site_argument_expressions(program, &source)
        .iter()
        .filter(|argument| matches!(program.expression_table.expression(**argument),
            ExpressionNode::Borrow(borrow) if borrow.access == language_semantics::ReferenceAccess::Shared && borrow.target == expression))
        .count() != 1
    {
        return None;
    }
    let producer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: producer.call_ordinal,
        target_symbol: producer.target_symbol,
    };
    let consumer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: consumer.call_ordinal,
        target_symbol: consumer.target_symbol,
    };
    let provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: producer_source,
    };
    let mut events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.root == facts::PlaceRoot::Expression(expression)
        });
    for (kind, access, multiplicity, source) in [
        (
            PermissionEventKind::Establish,
            PermissionAccess::Owned,
            Multiplicity::Affine,
            producer_source,
        ),
        (
            PermissionEventKind::Establish,
            PermissionAccess::Shared,
            Multiplicity::Unrestricted,
            consumer_source,
        ),
        (
            PermissionEventKind::AffineDrop,
            PermissionAccess::Owned,
            Multiplicity::Affine,
            consumer_source,
        ),
    ] {
        let event = events.next()?;
        if event.kind != kind
            || event.access != access
            || event.multiplicity != multiplicity
            || event.source != source
            || event.provenance != provenance
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span(event.segments)?
                .is_empty()
        {
            return None;
        }
    }
    events.next().is_none().then_some(())
}
