//! Component induction orchestrates the existing per-state arithmetic frames.
//! Candidates are proposals, not facts: all external and internal arrivals and
//! the implicit invocation must prove before any exit receives their authority.

use super::{AdmittedGuarantee, Candidate, Header};
use crate::checks::termination::{
    component_is_cyclic, machine_adjacency, strongly_connected_components,
};
use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use validation::ScopedArithmeticHypothesis;

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
    // Solve exact value origins once for the whole machine, including edges
    // between components. An unchanged parameter may remain the invocation
    // value even when another parameter of that same state drifts.
    let solved = crate::flow::machine_parameter_origins(program, machine);
    let origins = states
        .iter()
        .map(|state| crate::flow::state_parameter_origins(program, machine, state, &solved))
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    for component in strongly_connected_components(&adjacency) {
        if !component_is_cyclic(&adjacency, &component) {
            continue;
        }
        let Some(headers) = component
            .iter()
            .map(|&index| Header::new(program, facts, machine, &states[index], &origins))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let candidates = headers.iter().map(Header::candidates).collect::<Vec<_>>();
        if proves_component(&headers, &candidates) {
            admitted.extend(
                candidates
                    .into_iter()
                    .flatten()
                    .map(|candidate| candidate.exit),
            );
        }
    }
    admitted
}

fn proves_component(headers: &[Header<'_, '_>], candidates: &[Vec<Candidate>]) -> bool {
    for (header, destination_candidates) in headers.iter().zip(candidates) {
        let Some(arrivals) = header.arrivals() else {
            return false;
        };
        if arrivals.is_empty() {
            return false;
        }
        for arrival in arrivals {
            let mut hypotheses = header.requirements(header.formal_roster());
            if let Some(caller_index) = headers
                .iter()
                .position(|caller| caller.header.symbol == arrival.caller)
            {
                let caller = &headers[caller_index];
                hypotheses.extend(candidates[caller_index].iter().map(|candidate| {
                    ScopedArithmeticHypothesis {
                        proposition: caller.transported(candidate, arrival.caller_atoms.clone()),
                        holds: true,
                    }
                }));
            }
            // Entry requirements also describe the current entry frame after
            // reentry. Ordinary call-requirement checking establishes them on
            // every machine-name arrival; they are not invocation snapshots.
            if arrival.caller == header.entry.symbol {
                hypotheses.extend(header.requirements(arrival.caller_atoms.clone()));
            }
            hypotheses.extend(arrival.hypotheses.iter().map(|(expression, holds)| {
                header.hypothesis(*expression, arrival.caller_atoms.clone(), *holds)
            }));
            let destination =
                header.arrival_roster(&arrival.arguments, arrival.caller_atoms.clone());
            for candidate in destination_candidates {
                if !header.proves(
                    &hypotheses,
                    &header.transported(candidate, destination.clone()),
                ) {
                    return false;
                }
            }
        }
        if header.header.symbol == header.entry.symbol {
            let invocation = header.requirements(header.formal_roster());
            for candidate in destination_candidates {
                if !header.proves(
                    &invocation,
                    &header.transported(candidate, header.formal_roster()),
                ) {
                    return false;
                }
            }
        }
    }
    true
}
