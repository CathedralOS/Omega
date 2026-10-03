//! Shared access to an anonymous owner ends before that owner's call cleanup.
use super::{
    CheckFacts, FlowPermissionEventFact, HandleSpan, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionProvenance, TypeReferenceNode,
};
use crate::checks::type_carries_linear_obligation;
use crate::checks::type_multiplicity;
use language_semantics::PermissionEventSource;
use typed_trees::expression::ExpressionNode;

pub(in crate::checks::multiplicity) fn append_shared_borrow(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    permissions: &mut Vec<FlowPermissionEventFact>,
) {
    let Some((_, flow)) = facts.flow.control.states.iter().find(|(_, flow)| {
        flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
    }) else {
        return;
    };
    let Some(calls) = facts.flow.control.calls.span(flow.calls) else {
        return;
    };
    // Temporary ownership follows each borrowed operand's producer and normal
    // consumer completion. Neither statement spelling nor sibling call count
    // determines whether that owner exists.
    for consumer in calls {
        let Some((consumer_owner, _)) =
            crate::semantic::calls::find_state_with_machine(program, consumer.target_symbol)
        else {
            continue;
        };
        if consumer_owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            continue;
        }
        let Some(source) = crate::semantic::calls::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            consumer.statement_index,
            consumer.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) =
            crate::semantic::calls::call_target_parameters(program, consumer.target_symbol)
        else {
            continue;
        };
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &source);
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let parameters = parameters
            .iter()
            .filter(|parameter| !parameter.is_self || explicit_self);
        if parameters.clone().count() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.zip(arguments) {
            let Some(events) = events(
                program, machine, state, calls, consumer, parameter, *argument,
            ) else {
                continue;
            };
            if !permissions.iter().any(|event| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == events[0].root
            }) {
                permissions.extend(events);
            }
        }
    }
}

fn events(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    consumer: &checked_trees::FlowCallFact,
    parameter: &typed_trees::signature::StateParameter,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<[FlowPermissionEventFact; 3]> {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(argument) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    let mut producers = calls.iter().filter(|producer| {
        producer.statement_index == consumer.statement_index
            && producer.authored_expression == borrow.target
    });
    let producer = producers.next()?;
    if producers.next().is_some() || producer.call_ordinal <= consumer.call_ordinal {
        return None;
    }
    let ExpressionNode::Call(authored) = program.expression_table.expression(borrow.target) else {
        return None;
    };
    if authored.target_symbol != producer.target_symbol {
        return None;
    }
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
        || parameter.relevance.is_erased()
        || program.normalized_type_identity(*referee)
            != program.normalized_type_identity(produced_type)
        || type_multiplicity(program, produced_type) != Multiplicity::Affine
        || type_carries_linear_obligation(program, produced_type)
    {
        return None;
    }
    let producer_source = PermissionEventSource::Call {
        statement_index: producer.statement_index,
        call_ordinal: producer.call_ordinal,
        target_symbol: producer.target_symbol,
    };
    let consumer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: consumer.call_ordinal,
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
