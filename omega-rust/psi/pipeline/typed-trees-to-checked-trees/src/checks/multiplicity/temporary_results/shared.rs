//! Shared access to an anonymous owner ends before that owner's call cleanup.

use super::*;
use typed_trees::expression::ExpressionNode;

pub(in crate::checks::multiplicity) fn append_shared_borrow(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let Some(events) = events(program, facts, machine, state) else {
        return;
    };
    if permissions.iter().any(|event| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.root == events[0].root
    }) {
        return;
    }
    permissions.extend(events);
}

fn events(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<[FlowPermissionEventFact; 3]> {
    // No following statement may retain a temporary whose loan ends here.
    let [StatementNode::Call(_)] = program.statement_table.statements(state.statement_nodes) else {
        return None;
    };
    let flow = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .find(|flow| flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    if calls.len() != 2 || calls.iter().any(|call| call.statement_index != 0) {
        return None;
    }
    let consumer = calls.iter().find(|call| call.call_ordinal == 0)?;
    let producer = calls.iter().find(|call| call.call_ordinal == 1)?;
    let consumer_owner = program.machines().iter().find(|owner| {
        program
            .machine_states(owner)
            .iter()
            .any(|target| target.symbol == consumer.target_symbol)
    })?;
    if consumer_owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return None;
    }
    let result_type = crate::flow::call_target_return_type(program, consumer.target_symbol)?;
    if !matches!(
        program.type_reference_table.type_reference(result_type),
        TypeReferenceNode::Unit
    ) {
        return None;
    }
    let source = crate::find_call_site(program, machine.symbol, state.symbol, 0, 0)?;
    let [argument] = crate::call_site_argument_expressions(program, &source) else {
        return None;
    };
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(*argument) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared
        || borrow.target != producer.authored_expression
    {
        return None;
    }
    let ExpressionNode::Call(authored) = program.expression_table.expression(borrow.target) else {
        return None;
    };
    if authored.target_symbol != producer.target_symbol {
        return None;
    }
    let [parameter] = crate::call_target_parameters(program, consumer.target_symbol)? else {
        return None;
    };
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let produced_type = crate::flow::call_target_return_type(program, producer.target_symbol)?;
    if parameter.is_self
        || parameter.is_const
        || parameter.is_mutable
        || program.normalized_type_identity(*referee)
            != program.normalized_type_identity(produced_type)
        || type_multiplicity(program, produced_type) != Multiplicity::Affine
        || !validation::has_plain_owned_contents(program, produced_type)
        || type_carries_linear_obligation(program, produced_type)
    {
        return None;
    }
    let producer_source = PermissionEventSource::Call {
        statement_index: 0,
        call_ordinal: 1,
        target_symbol: producer.target_symbol,
    };
    let consumer_source = PermissionEventSource::Call {
        statement_index: 0,
        call_ordinal: 0,
        target_symbol: consumer.target_symbol,
    };
    let establishment = FlowPermissionEventFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        source: producer_source,
        kind: PermissionEventKind::Establish,
        multiplicity: Multiplicity::Affine,
        access: PermissionAccess::Owned,
        claim_identity: PermissionClaimIdentity::Unknown,
        provenance: PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: producer_source,
        },
        root: facts::PlaceRoot::Expression(borrow.target),
        segments: HandleSpan::empty(),
        obligation_live: false,
    };
    Some([
        establishment.clone(),
        FlowPermissionEventFact {
            source: consumer_source,
            multiplicity: Multiplicity::Unrestricted,
            access: PermissionAccess::Shared,
            ..establishment.clone()
        },
        FlowPermissionEventFact {
            source: consumer_source,
            kind: PermissionEventKind::AffineDrop,
            ..establishment
        },
    ])
}
