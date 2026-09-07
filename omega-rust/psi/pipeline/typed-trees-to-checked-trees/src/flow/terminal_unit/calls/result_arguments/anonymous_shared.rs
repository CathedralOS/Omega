//! A shared call retains and then disposes the actual temporary owner.

use super::*;

pub(super) fn validate(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<()> {
    let source_state = crate::find_state(program, state)?;
    let StatementNode::Call(_) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(consumer.statement_index)?
    else {
        return None;
    };
    if consumer.call_ordinal != 0 {
        return None;
    }
    let target = program.machines().iter().find(|target| {
        program
            .machine_states(target)
            .iter()
            .any(|target_state| target_state.symbol == consumer.target_symbol)
    })?;
    if target.supply_mode != MachineSupplyMode::CheckedBody
        || !is_unit(
            program,
            crate::flow::call_target_return_type(program, consumer.target_symbol)?,
        )
    {
        return None;
    }
    let flow = state_flow(facts, machine, state)?;
    let calls = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|call| call.statement_index == consumer.statement_index)
        .collect::<Vec<_>>();
    if calls.len() != 2 {
        return None;
    }
    let producer = calls
        .iter()
        .find(|call| call.call_ordinal == 1 && call.authored_expression == expression)?;
    let source = crate::find_call_site(program, machine, state, consumer.statement_index, 0)?;
    let [argument] = crate::call_site_argument_expressions(program, &source) else {
        return None;
    };
    if !matches!(program.expression_table.expression(*argument),
        ExpressionNode::Borrow(borrow) if borrow.access == language_semantics::ReferenceAccess::Shared && borrow.target == expression)
    {
        return None;
    }
    let producer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: 1,
        target_symbol: producer.target_symbol,
    };
    let consumer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: 0,
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
