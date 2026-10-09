//! Where an authored rank can still be read by recurrence.
//!
//! Rank membership ends on an exit suffix that cannot return to a ranked
//! cycle. This query follows the existing executable call inventory, including
//! ordinary and nested calls: tail-only topology would hide recursion inside
//! a finishing initializer. Opaque executable destinations remain relevant.
//! Relevance neither proves descent nor admits an operation; source and
//! executable rank consumers retain their existing formation and edge proofs.

use crate::machine_calls::calls::{
    CallSite, call_target_state, collect_call_sites_in_statement, find_state_with_machine,
};
use symbols::SymbolHandle;

/// Transient source topology shared by range arrivals and runtime projection.
pub struct RankRelevance {
    states: Vec<(SymbolHandle, bool)>,
}

impl RankRelevance {
    /// Absence is not evidence that an unrelated or unresolved state is an exit.
    pub fn requires_rank(&self, state: SymbolHandle) -> bool {
        self.states
            .iter()
            .find(|(symbol, _)| *symbol == state)
            .is_none_or(|(_, relevant)| *relevant)
    }
}

/// Retain every predecessor of a cyclic component containing a ranked owner's
/// state. Other callees' independent cycles do not read this authored rank.
/// Unknown destinations conservatively prevent their entire preceding suffix
/// from discarding its rank obligations.
pub fn rank_relevance(
    program: &dyn crate::CheckingInput,
    ranked_machines: &[SymbolHandle],
) -> Option<RankRelevance> {
    let mut states = Vec::<(&typed_trees::machine::Machine, &typed_trees::state::State)>::new();
    for symbol in ranked_machines {
        let machine = program.machine_by_symbol(*symbol)?;
        for state in program.machine_states(machine) {
            if !states
                .iter()
                .any(|(_, stored)| stored.symbol == state.symbol)
            {
                states.push((machine, state));
            }
        }
    }
    let mut adjacency = Vec::<Vec<usize>>::new();
    let mut opaque = Vec::new();
    let mut position = 0;
    while let Some(&(machine, state)) = states.get(position) {
        let mut targets = Vec::new();
        let mut has_opaque_call = false;
        for (statement_index, _) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            for call in collect_call_sites_in_statement(program, machine, state, statement_index) {
                let symbol = match call {
                    CallSite::Statement(call) => call.target_symbol,
                    CallSite::Expression { call, .. } => call.target_symbol,
                    CallSite::TransitionNamed { path, .. } => path.symbol,
                };
                let Some(target) = call_target_state(program, symbol) else {
                    has_opaque_call = true;
                    continue;
                };
                let Some((owner, target)) = find_state_with_machine(program, target.symbol) else {
                    has_opaque_call = true;
                    continue;
                };
                // A bodyless interface's implementation is not an empty body.
                if !owner.body_is_present {
                    has_opaque_call = true;
                    continue;
                }
                let target_position = states
                    .iter()
                    .position(|(_, stored)| stored.symbol == target.symbol)
                    .unwrap_or_else(|| {
                        let target_position = states.len();
                        states.push((owner, target));
                        target_position
                    });
                targets.push(target_position);
            }
        }
        targets.sort_unstable();
        targets.dedup();
        adjacency.push(targets);
        opaque.push(has_opaque_call);
        position += 1;
    }
    let mut relevant = opaque;
    for component in super::strongly_connected_components(&adjacency) {
        let cyclic = component.len() > 1
            || component
                .first()
                .is_some_and(|position| adjacency[*position].contains(position));
        if cyclic
            && component
                .iter()
                .any(|position| ranked_machines.contains(&states[*position].0.symbol))
        {
            for position in component {
                relevant[position] = true;
            }
        }
    }
    let mut predecessors = vec![Vec::new(); states.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for target in targets {
            predecessors[*target].push(source);
        }
    }
    let mut ready = relevant
        .iter()
        .enumerate()
        .filter_map(|(position, relevant)| relevant.then_some(position))
        .collect::<Vec<_>>();
    let mut position = 0;
    while let Some(&target) = ready.get(position) {
        for source in &predecessors[target] {
            if !relevant[*source] {
                relevant[*source] = true;
                ready.push(*source);
            }
        }
        position += 1;
    }
    Some(RankRelevance {
        states: states
            .into_iter()
            .zip(relevant)
            .map(|((_, state), relevant)| (state.symbol, relevant))
            .collect(),
    })
}
