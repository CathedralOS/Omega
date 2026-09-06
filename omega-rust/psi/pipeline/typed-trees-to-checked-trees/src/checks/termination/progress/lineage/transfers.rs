//! Exact parameter dependencies, with projection growth detected on cycles.

use super::{FlowFacts, ProgressSubject};
use crate::checks::termination::progress::{
    call_argument_subject_with_parameters, local_state_transition_target,
};

#[cfg(test)]
mod tests;

pub(super) struct ParameterTransfer {
    pub(super) destination: ProgressSubject,
    pub(super) source: Option<ProgressSubject>,
}

pub(super) fn collect(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
    subjects: &[ProgressSubject],
) -> Vec<ParameterTransfer> {
    let mut transfers = Vec::new();
    for (_, state) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
    {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let Some(target) = local_state_transition_target(program, machine, state, call) else {
                continue;
            };
            let parameters = program.state_parameters(target);
            for destination in subjects.iter().filter(|subject| {
                parameters
                    .iter()
                    .any(|parameter| parameter.symbol == subject.root)
            }) {
                transfers.push(ParameterTransfer {
                    destination: destination.clone(),
                    source: call_argument_subject_with_parameters(
                        program,
                        machine,
                        state,
                        call,
                        parameters,
                        destination.root,
                    )
                    .and_then(|mut subject| {
                        subject
                            .projections
                            .extend_from_slice(&destination.projections);
                        crate::checks::termination::progress::origins::at_call(
                            program, flow, machine, state, call, subject,
                        )
                    }),
                });
            }
        }
    }
    transfers
}

pub(super) fn grows_on_cycle(
    transfers: &[ParameterTransfer],
    candidate: &ParameterTransfer,
    subjects: &[ProgressSubject],
) -> bool {
    let Some(source) = &candidate.source else {
        return false;
    };
    let Some(source_place) = super::places::matching_prefix(subjects.iter(), source) else {
        return false;
    };
    if source.projections.len() == source_place.projections.len() {
        return false;
    }
    // Consume the source's most specific partition before testing growth.
    // Field swaps and other finite prefix replacements add no residual suffix.
    // The candidate adds a nonempty suffix from source to destination.
    // It grows on a cycle exactly when destination can reach source again.
    // Zero-projection cycles and acyclic projected transfers remain exact.
    let mut pending = vec![candidate.destination.clone()];
    let mut visited = Vec::new();
    while let Some(parameter) = pending.pop() {
        if parameter == *source_place {
            return true;
        }
        if visited.contains(&parameter) {
            continue;
        }
        visited.push(parameter.clone());
        for transfer in transfers {
            if transfer.source.as_ref().is_some_and(|source| {
                super::places::matching_prefix(subjects.iter(), source) == Some(&parameter)
            }) {
                pending.push(transfer.destination.clone());
            }
        }
    }
    false
}
