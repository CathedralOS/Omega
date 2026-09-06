//! Exact parameter dependencies, with projection growth detected on cycles.

use super::{FlowFacts, ProgressSubject, SymbolHandle};
use crate::checks::termination::progress::{
    call_argument_subject_with_parameters, local_state_transition_target,
};

#[cfg(test)]
mod tests;

pub(super) struct ParameterTransfer {
    pub(super) destination: SymbolHandle,
    pub(super) source: Option<ProgressSubject>,
}

pub(super) fn collect(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
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
            for parameter in parameters {
                transfers.push(ParameterTransfer {
                    destination: parameter.symbol,
                    source: call_argument_subject_with_parameters(
                        program,
                        machine,
                        state,
                        call,
                        parameters,
                        parameter.symbol,
                    )
                    .and_then(|subject| {
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
) -> bool {
    let Some(source) = &candidate.source else {
        return false;
    };
    if source.projections.is_empty() {
        return false;
    }
    // The candidate adds a nonempty suffix from source to destination.
    // It grows on a cycle exactly when destination can reach source again.
    // Zero-projection cycles and acyclic projected transfers remain exact.
    let mut pending = vec![candidate.destination];
    let mut visited = Vec::new();
    while let Some(parameter) = pending.pop() {
        if parameter == source.root {
            return true;
        }
        if visited.contains(&parameter) {
            continue;
        }
        visited.push(parameter);
        for transfer in transfers {
            if transfer
                .source
                .as_ref()
                .is_some_and(|source| source.root == parameter)
            {
                pending.push(transfer.destination);
            }
        }
    }
    false
}
