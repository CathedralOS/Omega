//! Discover demands backwards through the component DAG, then prove forwards.
//! A destination's exact exit predicate can demand a relation from a preceding
//! component with no exit of its own. Transport retains scoped substitutions,
//! not new source expressions or guessed parameter correspondence. Internal
//! cycle edges are proved inductively rather than unrolled during discovery.
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
    discover(&headers, &components, &arrivals, &mut demands);

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

fn discover(
    headers: &[Header<'_, '_>],
    components: &[Vec<usize>],
    arrivals: &[Vec<Arrival>],
    demands: &mut [Vec<ScopedArithmeticExpression>],
) {
    let mut remaining = demands::TRANSPORT_WORK;
    // Destinations precede their predecessors in this order, so a propagated
    // demand is present before its owning component is visited. No cyclic
    // expansion is necessary or permitted here.
    for component in components {
        for &destination in component {
            for arrival in &arrivals[destination] {
                let Some(caller) = headers
                    .iter()
                    .position(|header| header.header.symbol == arrival.caller)
                else {
                    continue;
                };
                if component.contains(&caller) {
                    continue;
                }
                let replacements = headers[destination]
                    .arrival_roster(&arrival.arguments, headers[caller].current_roster());
                let (before, after) = demands.split_at_mut(destination.max(caller));
                let (destination_demands, caller_demands) = if destination < caller {
                    (&before[destination], &mut after[0])
                } else {
                    (&after[0], &mut before[caller])
                };
                for demand in destination_demands {
                    let Some(transported) = demands::substitute(
                        demand,
                        &headers[destination],
                        &replacements,
                        &mut remaining,
                    ) else {
                        return;
                    };
                    if !caller_demands.contains(&transported) {
                        caller_demands.push(transported);
                    }
                }
            }
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
