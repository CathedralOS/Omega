use crate::EmissionPlanningInput;
use omega_state_calls::StateCall;

/// Whether a state call ROUTED TO DISPATCH: a runtime-flow call edge exists
/// for its source state + statement targeting the callee's machine.
/// Evidence-based (the route the state-graph builder actually recorded via
/// `dispatch_state_call_edges`), not a re-derived predicate, so fences using
/// this can never drift from the router.
///
/// A dispatched callee's states are REAL dispatch cases: arms run exactly
/// once when selected, loops are real back-edges, and value-position results
/// return through the clone terminal's `CallResultReturn`. The splice-only
/// hazards (all-arms execution, once-not-per-iteration interiors) do not
/// apply, so the splice fences exempt dispatched calls.
pub(crate) fn state_call_routed_to_dispatch(
    input: &EmissionPlanningInput<'_>,
    state_call: &StateCall,
) -> bool {
    let routed = input.runtime_flow.edges.iter().any(|(_, edge)| {
        edge.from.machine == state_call.source_key.machine
            && edge.from.state == state_call.source_key.state
            && edge.statement_index == state_call.statement_index
            && matches!(
                edge.target,
                omega_state_graph::RuntimeTransitionTarget::State { key, .. }
                    if key.machine == state_call.target_key.machine
            )
    });
    if !routed && std::env::var_os("OMEGA_DEBUG_DISPATCH_ROUTE").is_some() {
        let from_state_edges = input
            .runtime_flow
            .edges
            .iter()
            .filter(|(_, edge)| {
                edge.from.machine == state_call.source_key.machine
                    && edge.from.state == state_call.source_key.state
            })
            .count();
        let from_machine_edges = input
            .runtime_flow
            .edges
            .iter()
            .filter(|(_, edge)| edge.from.machine == state_call.source_key.machine)
            .count();
        eprintln!(
            "dispatch_route MISS: src machine {} state {} stmt {} -> target machine {}              (edges from same state: {}, same machine: {}, total: {})",
            state_call.source_key.machine.arena_index(),
            state_call.source_key.state.arena_index(),
            state_call.statement_index,
            state_call.target_key.machine.arena_index(),
            from_state_edges,
            from_machine_edges,
            input.runtime_flow.edges.len(),
        );
    }
    routed
}
