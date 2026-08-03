use super::*;

pub(crate) fn lower_proof_obligation(
    obligation: &psi_proof::obligations::ProofObligation,
) -> ProofObligationFact {
    match obligation {
        psi_proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::BoundedAssignment,
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
            }
        }
        psi_proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::BoundedCallArgument,
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                owner: ProofObligationOwner::CallParameter {
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    target_symbol: obligation.target_symbol,
                    parameter_symbol: obligation.parameter_symbol,
                },
            }
        }
        psi_proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::BoundedInitializer,
                machine_symbol: psi_symbols::SymbolHandle::invalid(),
                state_symbol: psi_symbols::SymbolHandle::invalid(),
                owner: proof_obligation_owner(&obligation.owner),
            }
        }
        psi_proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::BoundedStateReturn,
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                owner: ProofObligationOwner::StateReturn {
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                },
            }
        }
        psi_proof::obligations::ProofObligation::BoundedValue(obligation) => ProofObligationFact {
            kind: ProofFactKind::BoundedValue,
            machine_symbol: psi_symbols::SymbolHandle::invalid(),
            state_symbol: psi_symbols::SymbolHandle::invalid(),
            owner: proof_obligation_owner(&obligation.owner),
        },
        psi_proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::BoundedTransitionArgument,
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                owner: ProofObligationOwner::TransitionParameter {
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    parameter_symbol: obligation.parameter_symbol,
                },
            }
        }
        psi_proof::obligations::ProofObligation::GuardedTransition(obligation) => {
            ProofObligationFact {
                kind: ProofFactKind::GuardedTransition,
                machine_symbol: obligation.machine_symbol,
                state_symbol: obligation.state_symbol,
                owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
            }
        }
    }
}

pub(crate) fn state_owner(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> ProofObligationOwner {
    ProofObligationOwner::MachineState {
        machine_symbol,
        state_symbol,
    }
}

pub(crate) fn proof_obligation_owner(
    owner: &psi_proof::obligations::ProofObligationOwner,
) -> ProofObligationOwner {
    match owner {
        psi_proof::obligations::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        psi_proof::obligations::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            machine: _,
            data_symbol,
            data: _,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        psi_proof::obligations::ProofObligationOwner::StateParameter {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
            parameter_symbol,
            parameter: _,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        psi_proof::obligations::ProofObligationOwner::StateReturn {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
    }
}
