//! A shared call retains its temporary owner and discharges any affine debt.
use super::{
    ExpressionNode, Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::MachineSupplyMode;

pub(crate) fn validate(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
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
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &source);
    let parameters =
        crate::semantic::calls::call_target_parameters(program, consumer.target_symbol)?;
    if parameters.len() != arguments.len() {
        return None;
    }
    let mut borrowed_parameters =
        arguments
            .iter()
            .zip(parameters.iter())
            .filter_map(|(argument, parameter)| {
                let (referent, referee) = crate::semantic::calls::shared_argument_referent(
                    program,
                    *argument,
                    parameter.type_reference,
                )?;
                (referent == expression).then_some((parameter, referee))
            });
    let (parameter, referee) = borrowed_parameters.next()?;
    if borrowed_parameters.next().is_some()
        || parameter.is_const
        || parameter.is_mutable
        || parameter.relevance.is_erased()
    {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        consumer.statement_index,
        expression,
    )?;
    if place.root != facts::PlaceRoot::Expression(expression)
        || !place.segments.is_empty()
        || !validation::checked_argument_matches_type_reference(program, expression, referee)
        || !validation::has_borrow_free_contents(program, referee)
        || crate::checks::type_carries_linear_obligation(program, referee)
    {
        return None;
    }
    let multiplicity = crate::checks::type_multiplicity(program, referee);
    if !matches!(
        multiplicity,
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) {
        return None;
    }
    let producer_source =
        if let ExpressionNode::Call(authored) = program.expression_table.expression(expression) {
            let (_, state_flow) = flow.control.states.iter().find(|(_, candidate)| {
                candidate.machine_symbol == machine && candidate.state_symbol == state
            })?;
            let mut producers = flow
                .control
                .calls
                .span(state_flow.calls)?
                .iter()
                .filter(|call| {
                    call.statement_index == consumer.statement_index
                        && call.authored_expression == expression
                });
            let producer = producers.next()?;
            if producers.next().is_some()
                || producer.call_ordinal <= consumer.call_ordinal
                || crate::flow::call_target_symbol(program, authored) != producer.target_symbol
                || program.normalized_type_identity(crate::flow::call_target_return_type(
                    program,
                    producer.target_symbol,
                )?) != program.normalized_type_identity(referee)
            {
                return None;
            }
            PermissionEventSource::Call {
                statement_index: consumer.statement_index,
                call_ordinal: producer.call_ordinal,
                target_symbol: producer.target_symbol,
            }
        } else {
            PermissionEventSource::Statement {
                statement_index: consumer.statement_index,
            }
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
    let (source_machine, source_state) =
        crate::semantic::calls::find_state_with_machine(program, state)?;
    if source_machine.symbol != machine {
        return None;
    }
    let evaluation = crate::flow::call_permission_evaluation(
        program,
        source_machine,
        source_state,
        consumer.statement_index,
        consumer.call_ordinal,
    );
    if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
        return None;
    }
    if let PermissionEventSource::Call {
        statement_index,
        call_ordinal,
        ..
    } = producer_source
        && crate::flow::call_permission_evaluation(
            program,
            source_machine,
            source_state,
            statement_index,
            call_ordinal,
        ) != evaluation
    {
        return None;
    }
    // An unrestricted completed value has no affine disposal obligation.
    // Exact producer, argument and plain-owned type admission above still
    // apply; no fictional permission or cleanup event is manufactured.
    if multiplicity == Multiplicity::Unrestricted {
        return Some(());
    }
    let mut events = flow
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
        if event.evaluation != evaluation
            || event.kind != kind
            || event.access != access
            || event.multiplicity != multiplicity
            || event.source != source
            || event.provenance != provenance
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.obligation_live
            || !flow.ownership.segments.span(event.segments)?.is_empty()
        {
            return None;
        }
    }
    events.next().is_none().then_some(())
}
