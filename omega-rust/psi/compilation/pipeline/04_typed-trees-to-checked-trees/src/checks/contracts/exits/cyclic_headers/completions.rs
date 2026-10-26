//! A proved activation invariant can justify a recursive completion without
//! importing the recursive contract. Every reachable normal-result producer
//! must have its own admitted obligation; an unsupported exit blocks closure.

use super::AdmittedGuarantee;
use checked_trees::{CheckFacts, ContractProofFactKind};
use typed_trees::{
    machine::Machine,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

pub(super) fn extend(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &Machine,
    adjacency: &[Vec<usize>],
    admitted: &mut Vec<AdmittedGuarantee>,
) {
    let states = program.machine_states(machine);
    let mut reachable = vec![false; states.len()];
    let mut pending = vec![0];
    while let Some(index) = pending.pop() {
        if index >= reachable.len() || reachable[index] {
            continue;
        }
        reachable[index] = true;
        pending.extend_from_slice(&adjacency[index]);
    }
    let in_body = |symbol| {
        states
            .iter()
            .enumerate()
            .any(|(index, state)| reachable[index] && state.symbol == symbol)
    };
    let outgoing = facts
        .flow
        .control
        .outgoing
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine.symbol && in_body(row.state_symbol))
        .collect::<Vec<_>>();
    // Missing control publication cannot establish an all-normal-return proof.
    if states.iter().enumerate().any(|(index, state)| {
        reachable[index] && !outgoing.iter().any(|row| row.state_symbol == state.symbol)
    }) {
        return;
    }
    let covers = |contract| {
        outgoing.iter().all(|row| {
            let Some(state) = states.iter().find(|state| state.symbol == row.state_symbol) else {
                return false;
            };
            let statement = program
                .statement_table
                .statement_at(state, row.statement_index);
            if let Some(StatementNode::Transition(transition)) = statement
                && matches!(transition.exit, TransitionExit::Crash(_))
            {
                return true;
            }
            if row.transition_target.is_valid()
                && let TransitionTargetNode::Named { path, .. } = program
                    .statement_table
                    .transition_target(row.transition_target)
                && validation::activation_target_state_index(program, machine, path.symbol)
                    .is_some()
            {
                return true;
            }
            admitted.contains(&AdmittedGuarantee {
                state_symbol: row.state_symbol,
                statement_index: row.statement_index,
                transition_target: row.transition_target,
                contract,
            })
        })
    };
    let mut completed = Vec::new();
    for (_, state) in
        facts.flow.control.states.iter().filter(|(_, state)| {
            state.machine_symbol == machine.symbol && in_body(state.state_symbol)
        })
    {
        for exit in facts.flow.control.exits.span_or_empty(state.exits) {
            if !exit.transition_target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named { path, .. } = program
                .statement_table
                .transition_target(exit.transition_target)
            else {
                continue;
            };
            if validation::activation_target_state_index(program, machine, path.symbol) != Some(0) {
                continue;
            }
            for reference in facts.proof.contract_fact_refs.span_or_empty(exit.ensures) {
                let contract = facts.proof.contract_facts.get(reference.fact);
                if contract.kind == ContractProofFactKind::Ensures && covers(contract.fact) {
                    completed.push(AdmittedGuarantee {
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                        transition_target: exit.transition_target,
                        contract: contract.fact,
                    });
                }
            }
        }
    }
    admitted.extend(completed);
}
