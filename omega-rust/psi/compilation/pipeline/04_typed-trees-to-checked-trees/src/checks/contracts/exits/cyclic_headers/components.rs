//! Discover demands along state-simple backward paths, then prove components.
//! A destination's exact exit predicate can demand a relation from a preceding
//! component with no exit of its own. Transport retains scoped substitutions,
//! not new source expressions or guessed parameter correspondence. Internal
//! paths stop before revisiting a state: discovery can reach an exitless
//! intermediary inside a cycle, but never unrolls that cycle. Cycle edges still
//! require induction proofs, independently of how their demands were found.
//!
//! Each component proves all arrivals and invocation establishment before its
//! predicates become available to later components. Acyclic singleton bridges
//! follow the same rule. Within a component only its caller-state conjunction
//! is assumed; externally only already proved predicates may be imported.

use super::demands;
use super::{AdmittedGuarantee, Arrival, Header};
use crate::checks::termination::{machine_adjacency, strongly_connected_components};
use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use validation::{ScopedArithmeticExpression, ScopedArithmeticHypothesis};

pub(super) fn admitted(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
) -> Vec<AdmittedGuarantee> {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return Vec::new();
    };
    let states = program.machine_states(machine);
    let adjacency = machine_adjacency(program, machine);
    let components = strongly_connected_components(&adjacency);
    let solved = crate::flow::machine_parameter_origins(program, machine);
    let origins = states
        .iter()
        .map(|state| crate::flow::state_parameter_origins(program, machine, state, &solved))
        .collect::<Vec<_>>();
    let Some(headers) = states
        .iter()
        .map(|state| Header::new(program, facts, machine, state, &origins))
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    let Some(arrivals) = headers
        .iter()
        .map(Header::arrivals)
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    let candidates = headers.iter().map(Header::candidates).collect::<Vec<_>>();
    let mut demands = headers
        .iter()
        .zip(&candidates)
        .map(|(header, candidates)| {
            candidates
                .iter()
                .map(|candidate| header.transported(candidate, header.current_roster()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    discover(&headers, &arrivals, &mut demands);

    let mut proved = vec![false; states.len()];
    let mut admitted = Vec::new();
    // Shared Tarjan output is reverse topological. Prove predecessors before
    // permitting their relations as external-arrival premises.
    for component in components.iter().rev() {
        if proves_component(&headers, component, &arrivals, &demands, &proved) {
            for &index in component {
                proved[index] = true;
                admitted.extend(candidates[index].iter().map(|candidate| candidate.exit));
            }
        }
    }
    admitted
}

/// A demand occurrence keeps its own path: another path reaching the same
/// predicate can have a different unvisited predecessor. Predicate sharing in
/// the roster must not suppress that distinct discovery route.
struct PendingDemand {
    state: usize,
    predicate: usize,
    visited: Vec<usize>,
}

fn discovery_work(remaining: &mut usize, amount: usize) -> Option<()> {
    *remaining = remaining.checked_sub(amount)?;
    Some(())
}

fn discover(
    headers: &[Header<'_, '_>],
    arrivals: &[Vec<Arrival>],
    demands: &mut [Vec<ScopedArithmeticExpression>],
) {
    let mut remaining = demands::TRANSPORT_WORK;
    let mut pending = Vec::new();
    for (state, predicates) in demands.iter().enumerate() {
        for predicate in 0..predicates.len() {
            if discovery_work(&mut remaining, 1).is_none() {
                return;
            }
            pending.push(PendingDemand {
                state,
                predicate,
                visited: vec![state],
            });
        }
    }
    // FIFO traversal explores every original seed and shortest transport
    // first. A longer simple path cannot monopolize discovery ahead of a
    // nearer exit. The same finite budget pays for paths and substitutions.
    let mut cursor = 0;
    while cursor < pending.len() {
        let item = &pending[cursor];
        if discovery_work(&mut remaining, item.visited.len() + 1).is_none() {
            return;
        }
        let destination = item.state;
        let predicate = item.predicate;
        let visited = item.visited.clone();
        cursor += 1;
        for arrival in &arrivals[destination] {
            if discovery_work(&mut remaining, headers.len() + visited.len()).is_none() {
                return;
            }
            let Some(caller) = headers
                .iter()
                .position(|header| header.header.symbol == arrival.caller)
            else {
                continue;
            };
            if visited.contains(&caller) {
                continue;
            }
            let replacements = headers[destination]
                .arrival_roster(&arrival.arguments, headers[caller].current_roster());
            let Some(transported) = demands::substitute(
                &demands[destination][predicate],
                &headers[destination],
                &replacements,
                &mut remaining,
            ) else {
                return;
            };
            let predicate = match demands[caller]
                .iter()
                .position(|existing| existing == &transported)
            {
                Some(index) => index,
                None => {
                    demands[caller].push(transported);
                    demands[caller].len() - 1
                }
            };
            if discovery_work(&mut remaining, visited.len() + 1).is_none() {
                return;
            }
            let mut next_visited = visited.clone();
            next_visited.push(caller);
            pending.push(PendingDemand {
                state: caller,
                predicate,
                visited: next_visited,
            });
        }
    }
}

fn proves_component(
    headers: &[Header<'_, '_>],
    component: &[usize],
    arrivals: &[Vec<Arrival>],
    demands: &[Vec<ScopedArithmeticExpression>],
    proved: &[bool],
) -> bool {
    let mut remaining = demands::TRANSPORT_WORK;
    for &destination in component {
        let header = &headers[destination];
        if arrivals[destination].is_empty() && header.header.symbol != header.entry.symbol {
            return false;
        }
        for arrival in &arrivals[destination] {
            let mut hypotheses = header.requirements(header.formal_roster());
            if let Some(caller) = headers
                .iter()
                .position(|header| header.header.symbol == arrival.caller)
                && (component.contains(&caller) || proved[caller])
            {
                for demand in &demands[caller] {
                    let Some(proposition) = demands::substitute(
                        demand,
                        &headers[caller],
                        &arrival.caller_atoms,
                        &mut remaining,
                    ) else {
                        return false;
                    };
                    hypotheses.push(ScopedArithmeticHypothesis {
                        proposition,
                        holds: true,
                    });
                }
            }
            // Ordinary call checking re-establishes the entry requirements on
            // reentry. This is current-frame evidence, not an entry snapshot.
            if arrival.caller == header.entry.symbol {
                hypotheses.extend(header.requirements(arrival.caller_atoms.clone()));
            }
            hypotheses.extend(arrival.hypotheses.iter().map(|(expression, holds)| {
                header.hypothesis(*expression, arrival.caller_atoms.clone(), *holds)
            }));
            let replacements =
                header.arrival_roster(&arrival.arguments, arrival.caller_atoms.clone());
            for demand in &demands[destination] {
                let Some(goal) = demands::substitute(demand, header, &replacements, &mut remaining)
                else {
                    return false;
                };
                if !header.proves(&hypotheses, &goal) {
                    return false;
                }
            }
        }
        if header.header.symbol == header.entry.symbol {
            let invocation = header.requirements(header.formal_roster());
            for demand in &demands[destination] {
                let Some(goal) =
                    demands::substitute(demand, header, &header.formal_roster(), &mut remaining)
                else {
                    return false;
                };
                if !header.proves(&invocation, &goal) {
                    return false;
                }
            }
        }
    }
    true
}
