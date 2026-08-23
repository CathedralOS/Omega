use omega_control_flow::{InvariantFact, ProofFactKind, ProofObligationFact, ProofObligationOwner};

pub(crate) fn remap_proof_obligation_owned(
    obligation: omega_state_graph::ProofObligationFact,
) -> ProofObligationFact {
    ProofObligationFact {
        kind: remap_proof_kind(obligation.kind),
        machine_symbol: obligation.machine_symbol,
        state_symbol: obligation.state_symbol,
        owner: remap_proof_owner(&obligation.owner),
    }
}

pub(crate) fn remap_invariant_owned(invariant: omega_state_graph::InvariantFact) -> InvariantFact {
    InvariantFact {
        symbol: invariant.symbol,
        name: invariant.name,
        constraint_count: invariant.constraint_count,
    }
}

fn remap_proof_kind(kind: omega_state_graph::ProofFactKind) -> ProofFactKind {
    match kind {
        omega_state_graph::ProofFactKind::BoundedAssignment => ProofFactKind::BoundedAssignment,
        omega_state_graph::ProofFactKind::BoundedCallArgument => ProofFactKind::BoundedCallArgument,
        omega_state_graph::ProofFactKind::BoundedInitializer => ProofFactKind::BoundedInitializer,
        omega_state_graph::ProofFactKind::BoundedStateReturn => ProofFactKind::BoundedStateReturn,
        omega_state_graph::ProofFactKind::BoundedValue => ProofFactKind::BoundedValue,
        omega_state_graph::ProofFactKind::BoundedTransitionArgument => {
            ProofFactKind::BoundedTransitionArgument
        }
        omega_state_graph::ProofFactKind::GuardedTransition => ProofFactKind::GuardedTransition,
    }
}

fn remap_proof_owner(owner: &omega_state_graph::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_state_graph::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_state_graph::ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::MachineState {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_state_graph::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_state_graph::ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_state_graph::ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_state_graph::ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol,
            parameter_symbol,
        } => ProofObligationOwner::CallParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            target_symbol: *target_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_state_graph::ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::TransitionParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
    }
}
