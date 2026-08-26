use omega_state_graph::{
    StateContractCall, StateContractExit, StateContractFactKind, StateContractFactRef,
    StateContractSummary, StateGraph,
};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::CheckedTrees;

pub(crate) fn state_contract_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segment: &crate::segments::StateSegment,
    segment_transitions: &Arena<crate::segments::SegmentTransition>,
) -> StateContractSummary {
    let mut calls = HandleSpan::empty();
    for (_, call) in program.facts.proof.contract_calls.iter() {
        if call.caller_machine_symbol != segment.key.machine
            || call.caller_state_symbol != segment.key.state
            || !segment_contains_statement_index(
                state_graph,
                segment,
                segment_transitions,
                call.statement_index,
            )
        {
            continue;
        }

        let requires = append_state_contract_fact_refs(state_graph, program, call.requires);
        let ensures = append_state_contract_fact_refs(state_graph, program, call.ensures);
        state_graph.semantics.contracts.calls.append_to_span(
            &mut calls,
            StateContractCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
                requires,
                ensures,
            },
        );
    }

    let mut exits = HandleSpan::empty();
    for (_, exit) in program.facts.proof.contract_exits.iter() {
        if exit.machine_symbol != segment.key.machine
            || exit.state_symbol != segment.key.state
            || !segment_contains_statement_index(
                state_graph,
                segment,
                segment_transitions,
                exit.statement_index,
            )
        {
            continue;
        }

        let ensures = append_state_contract_fact_refs(state_graph, program, exit.ensures);
        state_graph.semantics.contracts.exits.append_to_span(
            &mut exits,
            StateContractExit {
                statement_index: exit.statement_index,
                ensures,
            },
        );
    }

    StateContractSummary { calls, exits }
}

fn segment_contains_statement_index(
    state_graph: &StateGraph,
    segment: &crate::segments::StateSegment,
    segment_transitions: &Arena<crate::segments::SegmentTransition>,
    statement_index: usize,
) -> bool {
    state_graph
        .operations
        .span_or_empty(segment.operations)
        .iter()
        .any(|operation| operation.statement_index == statement_index)
        || segment_transitions
            .span_or_empty(segment.transitions)
            .iter()
            .any(|transition| match transition {
                crate::segments::SegmentTransition::Tree {
                    statement_index: transition_statement_index,
                    ..
                }
                | crate::segments::SegmentTransition::ReturnExpression {
                    statement_index: transition_statement_index,
                    ..
                }
                | crate::segments::SegmentTransition::BranchCall {
                    statement_index: transition_statement_index,
                    ..
                } => *transition_statement_index == statement_index,
            })
}

fn append_state_contract_fact_refs(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    refs: HandleSpan<psi_checked_trees::ContractProofFactRef>,
) -> HandleSpan<StateContractFactRef> {
    let mut fact_refs = HandleSpan::empty();

    for reference in program.facts.proof.contract_fact_refs.span_or_empty(refs) {
        let contract_fact = program.facts.proof.contract_facts.get(reference.fact);
        state_graph.semantics.contracts.fact_refs.append_to_span(
            &mut fact_refs,
            StateContractFactRef {
                kind: match contract_fact.kind {
                    psi_checked_trees::ContractProofFactKind::Requires => {
                        StateContractFactKind::Requires
                    }
                    psi_checked_trees::ContractProofFactKind::Ensures => {
                        StateContractFactKind::Ensures
                    }
                },
                fact: contract_fact.fact,
            },
        );
    }

    fact_refs
}
