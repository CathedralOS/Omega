//! A shared call retains its complete temporary owner through normal completion.
//! Admission rejoins the published loan and each exact claim occurrence;
//! source checking owns eligibility and contextual hook requirements.
//! The completed producer supplies its retained storage type. Re-inferring
//! that type from a literal's leaves loses contextual construction identity;
//! the receiving reference formal must not supply a replacement owner type.
use super::{Multiplicity, SymbolHandle};
use crate::execution::terminal_unit::MachineSupplyMode;
use crate::execution::terminal_unit::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
};

pub(crate) fn validate(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    owner_type: typed_trees::types::TypeReferenceHandle,
) -> Option<()> {
    let target = program.machine_by_symbol(
        validation::find_state(program, consumer.target_symbol)?
            .0
            .symbol,
    )?;
    if target.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine,
        state,
        consumer.statement_index,
        consumer.call_ordinal,
    )?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    let parameters = validation::call_target_parameters(program, consumer.target_symbol)?;
    if parameters.len() != arguments.len() {
        return None;
    }
    let root = facts::PlaceRoot::Expression(expression);
    let mut operands =
        arguments
            .iter()
            .zip(parameters.iter())
            .filter_map(|(argument, parameter)| {
                if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
                    return None;
                }
                let (borrowed, _) = crate::semantic::calls::shared_argument_referent(
                    program,
                    *argument,
                    parameter.type_reference,
                )?;
                let place = validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    consumer.statement_index,
                    borrowed,
                )?;
                (place.root == root).then_some(place)
            });
    let borrowed = operands.next()?;
    if operands.next().is_some() {
        return None;
    }
    // Construction has already admitted each reference leaf and retained its
    // exact source. Lending the containing home neither copies those referents
    // nor transports their permissions into the callee. A borrow-free contents
    // whitelist here rejected ordinary composition instead of checking the
    // actual owner/loan/disposition relationships retained below and on calls.
    let multiplicity = crate::checks::type_multiplicity(program, owner_type);
    let (source_machine, source_state) = validation::find_state(program, state)?;
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
    let producer_source = if let Some(_) = program.expression_table.call_node(expression) {
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
            || crate::flow::call_permission_evaluation(
                program,
                source_machine,
                source_state,
                producer.statement_index,
                producer.call_ordinal,
            ) != evaluation
        {
            return None;
        }
        PermissionEventSource::Call {
            statement_index: producer.statement_index,
            call_ordinal: producer.call_ordinal,
            target_symbol: producer.target_symbol,
        }
    } else {
        PermissionEventSource::Statement {
            statement_index: consumer.statement_index,
        }
    };
    if multiplicity == Multiplicity::Unrestricted {
        return Some(());
    }
    if !crate::flow::whole_cleanup_is_eligible(program, owner_type) {
        return None;
    }
    let consumer_source = PermissionEventSource::Call {
        statement_index: consumer.statement_index,
        call_ordinal: consumer.call_ordinal,
        target_symbol: consumer.target_symbol,
    };
    let events = flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine && event.state_symbol == state && event.root == root
        })
        .collect::<Vec<_>>();
    if events.iter().any(|event| event.evaluation != evaluation) {
        return None;
    }
    let mut loans = events
        .iter()
        .copied()
        .filter(|event| event.access == PermissionAccess::Shared);
    let loan = loans.next()?;
    if loans.next().is_some()
        || loan.kind != PermissionEventKind::Establish
        || loan.source != consumer_source
        || loan.multiplicity != Multiplicity::Unrestricted
        || loan.claim_identity != PermissionClaimIdentity::Unknown
        || loan.obligation_live
        || flow.ownership.segments.span(loan.segments)? != borrowed.segments.as_slice()
    {
        return None;
    }
    let frontier = validation::linear_claim_frontier(program, owner_type);
    let paths = if frontier.is_empty() {
        vec![(Vec::new(), Multiplicity::Affine, false)]
    } else {
        frontier
            .into_iter()
            .map(|claim| (claim.path, Multiplicity::Linear, claim.conditional))
            .collect()
    };
    let mut covered = 1;
    for (path, claim_multiplicity, conditional) in paths {
        let mut establishments = events.iter().copied().filter(|event| {
            event.access == PermissionAccess::Owned
                && event.kind == PermissionEventKind::Establish
                && flow.ownership.segments.span_or_empty(event.segments) == path
        });
        let establishment = establishments.next()?;
        if establishments.next().is_some()
            || establishment.source != producer_source
            || establishment.multiplicity != claim_multiplicity
            || (claim_multiplicity == Multiplicity::Linear
                && !establishment.obligation_live
                && !conditional)
            || (establishment.obligation_live
                && establishment.claim_identity == PermissionClaimIdentity::Unknown)
        {
            return None;
        }
        covered += 1;
        if claim_multiplicity == Multiplicity::Linear && !establishment.obligation_live {
            continue;
        }
        let mut deaths = events.iter().copied().filter(|event| {
            event.access == PermissionAccess::Owned
                && event.kind
                    == if claim_multiplicity == Multiplicity::Linear {
                        PermissionEventKind::Consume
                    } else {
                        PermissionEventKind::AffineDrop
                    }
                && flow.ownership.segments.span_or_empty(event.segments) == path
        });
        let death = deaths.next()?;
        if deaths.next().is_some()
            || death.source != consumer_source
            || death.multiplicity != claim_multiplicity
            || death.obligation_live
            || death.claim_identity != establishment.claim_identity
            || death.provenance != establishment.provenance
        {
            return None;
        }
        covered += 1;
    }
    (covered == events.len()).then_some(())
}
