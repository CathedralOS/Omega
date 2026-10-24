//! Where an authored rank can still be read by recurrence.
//!
//! Rank membership ends on an exit suffix that cannot return to a ranked
//! cycle. This query follows the existing executable call inventory, including
//! ordinary and nested calls: tail-only topology would hide recursion inside
//! a finishing initializer. A pinned boundary with an empty synchronous
//! invocation ceiling cannot return to the ranked component. Its progress
//! guarantee is independent: an exit suffix may discard the rank even when
//! that boundary has no termination promise. Unresolved destinations and
//! boundaries permitting synchronous invocation remain relevant.
//! Body-bearing token operators have already been bound to exact call nodes
//! by typed preparation. Implicit cleanup has its own cycle owner, which rejects
//! recurrence without an exact cleanup-edge ranking witness. Those admission
//! boundaries keep hidden invocations from masquerading as an acyclic suffix.
//! Relevance neither proves descent nor admits an operation; source and
//! executable rank consumers retain their existing formation and edge proofs.

use crate::machine_calls::calls::{call_target_state, collect_call_sites_in_statement, find_state};
use crate::machine_calls::graph_components::strongly_connected_components;
use symbols::SymbolHandle;

/// Transient source topology shared by range arrivals and runtime projection.
pub struct RankRelevance {
    states: Vec<(SymbolHandle, bool)>,
}

impl RankRelevance {
    /// Absence is not evidence that an unrelated or unresolved state is an exit.
    pub fn requires_rank(&self, state: SymbolHandle) -> bool {
        crate::substitutions::actual_for(&self.states, state).unwrap_or(true)
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
        // A generated interface carries no executable body. Its topology is
        // the union of all retained members, never the current host's member
        // or the interface's deliberately empty authored invocation roster.
        let callees = if machine.target_family_interface {
            match super::super::calls::target_family_members(program, machine) {
                Some(members) => members.iter().map(|member| member.machine).collect(),
                None => {
                    has_opaque_call = true;
                    Vec::new()
                }
            }
        } else {
            program
                .statement_table
                .indexed_statements(state.statement_nodes)
                .flat_map(|(statement_index, _)| {
                    collect_call_sites_in_statement(program, machine, state, statement_index)
                })
                .map(|call| call.target_symbol(program))
                .collect::<Vec<_>>()
        };
        for symbol in callees {
            if boundary_excludes_synchronous_entry(program, symbol) {
                continue;
            }
            let Some(target) = call_target_state(program, symbol) else {
                has_opaque_call = true;
                continue;
            };
            let Some((owner, target)) = find_state(program, target.symbol) else {
                has_opaque_call = true;
                continue;
            };
            // A bodyless interface's implementation is not an empty body.
            // Generated family nodes expand their complete roster above.
            if !owner.body_is_present && !owner.target_family_interface {
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
        targets.sort_unstable();
        targets.dedup();
        adjacency.push(targets);
        opaque.push(has_opaque_call);
        position += 1;
    }
    let mut relevant = opaque;
    for component in strongly_connected_components(&adjacency) {
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

/// Bodyless omission forbids synchronous entry (language/effects.md). Read the
/// exact authored ceiling, not a filtered inference result: an unresolved
/// `invokes` target must not become evidence of an empty ceiling. Ordinary
/// requirements lack this boundary guarantee and retain opaque topology.
fn boundary_excludes_synchronous_entry(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> bool {
    if let Some((owner, signature)) = program.trait_machine_signature(target) {
        return owner.is_boundary && signature.invokes.is_empty();
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        // The boundary operator contract has no synchronous-invocation row.
        return operator.is_boundary;
    }
    let Some((machine, _)) = super::super::calls::find_machine_head(program, target) else {
        return false;
    };
    if machine.body_is_present
        || !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::TopLevelRequirement
                | language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
        || !machine.invokes.is_empty()
    {
        return false;
    }
    // A directly named external satisfier retains every pinned requirement's
    // ceiling; a narrower declaration cannot erase an inherited callback edge.
    let requirements = program.machine_trait_conformances(machine);
    (!matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) || !requirements.is_empty())
        && requirements.iter().all(|conformance| {
            match typed_trees::machine::resolve_satisfied_declaration(
                program.typed_program(),
                machine,
                conformance,
            ) {
                Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                }) => definition.is_boundary && requirement.invokes.is_empty(),
                Some(typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                    requirement,
                )) => requirement.invokes.is_empty(),
                Some(typed_trees::machine::SatisfiedDeclaration::Operator(operator)) => {
                    operator.is_boundary
                }
                _ => false,
            }
        })
}
