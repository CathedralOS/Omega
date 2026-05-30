use omega_checked_trees::FlowFacts;
use omega_core::arena::Arena;
use omega_effects::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, EffectPlan};
use omega_typed_trees::TypedTrees;

pub(crate) fn build_capability_facts(
    _program: &TypedTrees,
    _effects: &EffectPlan,
    flow: &FlowFacts,
) -> CapabilityFlowPlan {
    let mut flows = Arena::with_capacity(flow.boundaries.edges.len());

    for (_, state) in flow.control.states.iter() {
        for call in flow.control.calls.span_or_empty(state.calls) {
            for edge in flow.boundaries.edges.span_or_empty(call.boundary_edges) {
                flows.append(CapabilityFlowFact {
                    kind: CapabilityFlowKind::Uses,
                    capability_symbol: edge.boundary_trait_symbol,
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: edge.statement_index,
                    call_ordinal: edge.call_ordinal,
                });
            }
        }
    }

    CapabilityFlowPlan::with_roots(flows)
}
