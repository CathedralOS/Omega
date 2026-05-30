use omega_abstract_operations::{
    AbstractDropEvent, AbstractMoveEvent, AbstractOwnershipEventSource, AbstractOwnershipSummary,
};
use omega_control_flow::{ControlFlowPlan, StateOwnershipEventSource};

pub(super) fn build_abstract_ownership_summary(
    control_flow: &ControlFlowPlan,
) -> AbstractOwnershipSummary {
    let mut summary = AbstractOwnershipSummary::with_capacity(
        control_flow.semantics.ownership.segments.len(),
        control_flow.semantics.ownership.moves.len(),
        control_flow.semantics.ownership.drops.len(),
    );

    for (_, state) in control_flow.states.iter() {
        for event in control_flow
            .semantics
            .ownership
            .moves
            .span_or_empty(state.ownership.moves)
        {
            summary.moves.insert(AbstractMoveEvent {
                source_key: state.key,
                source: remap_ownership_event_source(event.source),
                root: event.root,
                segments: summary.segments.insert_many(
                    control_flow
                        .semantics
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            });
        }

        for event in control_flow
            .semantics
            .ownership
            .drops
            .span_or_empty(state.ownership.drops)
        {
            summary.drops.insert(AbstractDropEvent {
                source_key: state.key,
                source: remap_ownership_event_source(event.source),
                root: event.root,
                segments: summary.segments.insert_many(
                    control_flow
                        .semantics
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            });
        }
    }

    summary
}

fn remap_ownership_event_source(source: StateOwnershipEventSource) -> AbstractOwnershipEventSource {
    match source {
        StateOwnershipEventSource::Statement { statement_index } => {
            AbstractOwnershipEventSource::Statement { statement_index }
        }
        StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => AbstractOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        StateOwnershipEventSource::StateExit => AbstractOwnershipEventSource::StateExit,
    }
}

#[cfg(test)]
mod tests;
