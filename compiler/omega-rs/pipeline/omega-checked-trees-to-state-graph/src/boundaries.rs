use omega_state_graph::{StateBoundaryEdge, StateBoundarySummary, StateGraph, StateKey};
use psi_arena::HandleSpan;
use psi_checked_trees::{CheckedTrees, FlowBoundaryEdgeFact};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn state_boundary_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> Result<StateBoundarySummary, Diagnostic> {
    let typed_state = exact_typed_state(program, key.machine, key.state)?;
    let matching_flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.state_symbol == key.state)
        .map(|(_, state)| state)
        .collect::<Vec<_>>();
    let [flow_state] = matching_flow_states.as_slice() else {
        return Err(Diagnostic::error(
            "state-graph boundary FlowState coordinate is missing, duplicated, or cross-owned",
        ));
    };
    if flow_state.machine_symbol != key.machine {
        return Err(Diagnostic::error(
            "state-graph boundary FlowState coordinate is missing, duplicated, or cross-owned",
        ));
    }

    let state_edges = program
        .facts
        .flow
        .boundaries
        .edges
        .span(flow_state.boundary_edges)
        .ok_or_else(|| Diagnostic::error("state-graph boundary state edge span is invalid"))?;
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .ok_or_else(|| Diagnostic::error("state-graph boundary call span is invalid"))?;
    let statement_count = program
        .statement_table
        .statements(typed_state.statement_nodes)
        .len();
    let mut expected_edges = Vec::with_capacity(state_edges.len());
    for (call_index, call) in calls.iter().enumerate() {
        if call.statement_index >= statement_count {
            return Err(Diagnostic::error(
                "state-graph boundary call statement coordinate is out of range",
            ));
        }
        if calls[..call_index].iter().any(|prior| {
            prior.statement_index == call.statement_index && prior.call_ordinal == call.call_ordinal
        }) {
            return Err(Diagnostic::error(
                "state-graph boundary call coordinate is duplicated",
            ));
        }
        let call_edges = program
            .facts
            .flow
            .boundaries
            .edges
            .span(call.boundary_edges)
            .ok_or_else(|| Diagnostic::error("state-graph boundary call edge span is invalid"))?;
        for edge in call_edges {
            if edge.statement_index != call.statement_index
                || edge.call_ordinal != call.call_ordinal
                || edge.receiver_symbol != call.receiver_symbol
                || edge.target_symbol != call.target_symbol
            {
                return Err(Diagnostic::error(
                    "state-graph boundary edge disagrees with its exact call coordinate",
                ));
            }
            if expected_edges.iter().any(|prior| *prior == edge) {
                return Err(Diagnostic::error(
                    "state-graph boundary edge row is duplicated",
                ));
            }
            validate_boundary_requirement(program, edge)?;
            expected_edges.push(edge);
        }
    }
    if state_edges.len() != expected_edges.len()
        || state_edges
            .iter()
            .zip(&expected_edges)
            .any(|(state_edge, expected)| state_edge != *expected)
    {
        return Err(Diagnostic::error(
            "state-graph boundary state carrier is detached, reordered, or incomplete",
        ));
    }

    Ok(StateBoundarySummary {
        edges: append_boundary_edges(
            state_graph,
            &program.facts.flow.boundaries.edges,
            flow_state.boundary_edges,
        ),
    })
}

fn exact_typed_state<'program>(
    program: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::state::State, Diagnostic> {
    if !machine_symbol.is_valid() || !state_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph boundary typed owner coordinate is invalid",
        ));
    }
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == machine_symbol)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(Diagnostic::error(
            "state-graph boundary typed machine is missing or duplicated",
        ));
    };
    let owner_matches = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == state_symbol)
        .collect::<Vec<_>>();
    let all_matches = program
        .machines()
        .iter()
        .flat_map(|candidate| program.machine_states(candidate))
        .filter(|state| state.symbol == state_symbol)
        .count();
    match owner_matches.as_slice() {
        [state] if all_matches == 1 => Ok(*state),
        _ => Err(Diagnostic::error(
            "state-graph boundary typed state is missing, duplicated, or cross-owned",
        )),
    }
}

fn validate_boundary_requirement(
    program: &CheckedTrees,
    edge: &FlowBoundaryEdgeFact,
) -> Result<(), Diagnostic> {
    if !edge.boundary_trait_symbol.is_valid() || !edge.boundary_signature_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph boundary requirement identity is invalid",
        ));
    }
    let traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == edge.boundary_trait_symbol)
        .collect::<Vec<_>>();
    let [definition] = traits.as_slice() else {
        return Err(Diagnostic::error(
            "state-graph boundary trait is missing or duplicated",
        ));
    };
    if !definition.is_boundary {
        return Err(Diagnostic::error(
            "state-graph boundary edge names a non-boundary trait",
        ));
    }
    let owner_matches = program
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.symbol == edge.boundary_signature_symbol)
        .count();
    let all_matches = program
        .traits()
        .iter()
        .flat_map(|candidate| program.trait_machine_signatures(candidate))
        .filter(|signature| signature.symbol == edge.boundary_signature_symbol)
        .count();
    if owner_matches != 1 || all_matches != 1 {
        return Err(Diagnostic::error(
            "state-graph boundary signature is missing, duplicated, or cross-owned",
        ));
    }
    Ok(())
}

fn append_boundary_edges(
    state_graph: &mut StateGraph,
    source_edges: &psi_arena::Arena<FlowBoundaryEdgeFact>,
    edges: HandleSpan<FlowBoundaryEdgeFact>,
) -> HandleSpan<StateBoundaryEdge> {
    state_graph.semantics.boundaries.edges.insert_many(
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
    source_edges: &psi_arena::Arena<StateBoundaryEdge>,
    summary: &StateBoundarySummary,
) -> StateBoundarySummary {
    StateBoundarySummary {
        edges: target
            .semantics
            .boundaries
            .edges
            .insert_many(source_edges.span_or_empty(summary.edges).iter().cloned()),
    }
}

#[cfg(test)]
mod tests;
