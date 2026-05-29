use omega_checked_trees::{CheckedTrees, FlowBoundaryEdgeFact};
use omega_core::arena::HandleSpan;
use omega_state_graph::{StateBoundaryEdge, StateBoundarySummary, StateGraph, StateKey};

pub(crate) fn state_boundary_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateBoundarySummary {
    let Some(flow_state) = program.facts.flow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == key.machine && state.state_symbol == key.state).then_some(state)
    }) else {
        return StateBoundarySummary::default();
    };

    StateBoundarySummary {
        edges: append_boundary_edges(
            state_graph,
            &program.facts.flow.boundary_edges,
            flow_state.boundary_edges,
        ),
    }
}

fn append_boundary_edges(
    state_graph: &mut StateGraph,
    source_edges: &omega_core::arena::Arena<FlowBoundaryEdgeFact>,
    edges: HandleSpan<FlowBoundaryEdgeFact>,
) -> HandleSpan<StateBoundaryEdge> {
    state_graph
        .boundary_edges
        .insert_many(
            source_edges
                .span_or_empty(edges)
                .iter()
                .map(|edge| StateBoundaryEdge {
                    statement_index: edge.statement_index,
                    call_ordinal: edge.call_ordinal,
                    receiver_symbol: edge.receiver_symbol,
                    target_symbol: edge.target_symbol,
                    boundary_trait_symbol: edge.boundary_trait_symbol,
                    boundary_signature_symbol: edge.boundary_signature_symbol,
                }),
        )
}

pub(crate) fn remap_state_boundary_summary(
    target: &mut StateGraph,
    source_edges: &omega_core::arena::Arena<StateBoundaryEdge>,
    summary: &StateBoundarySummary,
) -> StateBoundarySummary {
    StateBoundarySummary {
        edges: target
            .boundary_edges
            .insert_many(source_edges.span_or_empty(summary.edges).iter().cloned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn state_boundary_summary_keeps_edges_for_matching_state() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let other_state_symbol = SymbolHandle::from_arena_index(3);
        let boundary_trait_symbol = SymbolHandle::from_arena_index(4);
        let boundary_signature_symbol = SymbolHandle::from_arena_index(5);

        let mut program = CheckedTrees::default();
        let mut matching_edges = HandleSpan::empty();
        program.facts.flow.boundary_edges.append_to_span(
            &mut matching_edges,
            FlowBoundaryEdgeFact {
                statement_index: 7,
                call_ordinal: 1,
                receiver_symbol: machine_symbol,
                target_symbol: state_symbol,
                boundary_trait_symbol,
                boundary_signature_symbol,
            },
        );
        program
            .facts
            .flow
            .states
            .insert(omega_checked_trees::FlowStateFact {
                machine_symbol,
                state_symbol,
                boundary_edges: matching_edges,
                ..Default::default()
            });

        let mut other_edges = HandleSpan::empty();
        program.facts.flow.boundary_edges.append_to_span(
            &mut other_edges,
            FlowBoundaryEdgeFact {
                statement_index: 9,
                call_ordinal: 0,
                receiver_symbol: machine_symbol,
                target_symbol: other_state_symbol,
                boundary_trait_symbol,
                boundary_signature_symbol,
            },
        );
        program
            .facts
            .flow
            .states
            .insert(omega_checked_trees::FlowStateFact {
                machine_symbol,
                state_symbol: other_state_symbol,
                boundary_edges: other_edges,
                ..Default::default()
            });

        let mut state_graph = StateGraph::default();
        let summary = state_boundary_summary(
            &mut state_graph,
            &program,
            StateKey {
                machine: machine_symbol,
                state: state_symbol,
                segment_index: 0,
            },
        );

        let edges = state_graph.boundary_edges.span_or_empty(summary.edges);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].statement_index, 7);
        assert_eq!(edges[0].call_ordinal, 1);
        assert_eq!(edges[0].target_symbol, state_symbol);
        assert_eq!(edges[0].boundary_trait_symbol, boundary_trait_symbol);
        assert_eq!(
            edges[0].boundary_signature_symbol,
            boundary_signature_symbol
        );
    }
}
