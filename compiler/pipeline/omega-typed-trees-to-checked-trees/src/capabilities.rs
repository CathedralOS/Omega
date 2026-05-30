use omega_checked_trees::FlowFacts;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_effects::{CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, EffectPlan};
use omega_typed_trees::TypedTrees;

pub(crate) fn build_capability_facts(
    program: &TypedTrees,
    effects: &EffectPlan,
    flow: &FlowFacts,
) -> CapabilityFlowPlan {
    let mut flows = Arena::with_capacity(flow.boundaries.edges.len());

    for (_, state) in flow.control.states.iter() {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let mut appended_from_boundary_edges = false;
            for edge in flow.boundaries.edges.span_or_empty(call.boundary_edges) {
                flows.append(CapabilityFlowFact {
                    kind: CapabilityFlowKind::Uses,
                    capability_symbol: edge.boundary_trait_symbol,
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: edge.statement_index,
                    call_ordinal: edge.call_ordinal,
                });
                appended_from_boundary_edges = true;
            }

            if appended_from_boundary_edges {
                continue;
            }

            if let Some(capability_symbol) =
                boundary_trait_for_signature(program, call.target_symbol)
            {
                flows.append(CapabilityFlowFact {
                    kind: CapabilityFlowKind::Uses,
                    capability_symbol,
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                });
            }
        }
    }

    for machine in effects.machines() {
        for state in effects.states.span_or_empty(machine.states) {
            for call in effects.calls.span_or_empty(state.calls) {
                let Some(capability_symbol) =
                    boundary_trait_for_signature(program, call.target_state_symbol)
                else {
                    continue;
                };

                append_unique_use(
                    &mut flows,
                    CapabilityFlowFact {
                        kind: CapabilityFlowKind::Uses,
                        capability_symbol,
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                    },
                );
            }
        }
    }

    CapabilityFlowPlan::with_roots(flows)
}

fn append_unique_use(flows: &mut Arena<CapabilityFlowFact>, fact: CapabilityFlowFact) {
    let exists = flows.iter().any(|(_, existing)| {
        existing.kind == fact.kind
            && existing.capability_symbol == fact.capability_symbol
            && existing.machine_symbol == fact.machine_symbol
            && existing.state_symbol == fact.state_symbol
            && existing.statement_index == fact.statement_index
            && existing.call_ordinal == fact.call_ordinal
    });

    if !exists {
        flows.append(fact);
    }
}

fn boundary_trait_for_signature(
    program: &TypedTrees,
    signature_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !signature_symbol.is_valid() {
        return None;
    }

    program.traits().iter().find_map(|trait_definition| {
        (trait_definition.is_boundary
            && program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|signature| signature.symbol == signature_symbol))
        .then_some(trait_definition.symbol)
    })
}
