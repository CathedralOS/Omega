use omega_state_graph::{InvariantFact, ProofFactKind, ProofObligationFact, ProofObligationOwner};

pub(crate) fn remap_proof_obligations<'a>(
    fact_count: usize,
    facts: impl Iterator<Item = &'a omega_checked_trees::ProofObligationFact>,
) -> omega_core::arena::Arena<ProofObligationFact> {
    let mut obligations = omega_core::arena::Arena::with_capacity(fact_count);

    for fact in facts {
        obligations.append(ProofObligationFact {
            kind: match fact.kind {
                omega_checked_trees::ProofFactKind::BoundedAssignment => {
                    ProofFactKind::BoundedAssignment
                }
                omega_checked_trees::ProofFactKind::BoundedCallArgument => {
                    ProofFactKind::BoundedCallArgument
                }
                omega_checked_trees::ProofFactKind::BoundedInitializer => {
                    ProofFactKind::BoundedInitializer
                }
                omega_checked_trees::ProofFactKind::BoundedStateReturn => {
                    ProofFactKind::BoundedStateReturn
                }
                omega_checked_trees::ProofFactKind::BoundedValue => ProofFactKind::BoundedValue,
                omega_checked_trees::ProofFactKind::BoundedTransitionArgument => {
                    ProofFactKind::BoundedTransitionArgument
                }
                omega_checked_trees::ProofFactKind::GuardedTransition => {
                    ProofFactKind::GuardedTransition
                }
            },
            machine_symbol: fact.machine_symbol,
            state_symbol: fact.state_symbol,
            owner: remap_proof_owner(&fact.owner),
        });
    }

    obligations
}

fn remap_proof_owner(owner: &omega_checked_trees::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_checked_trees::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_checked_trees::ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::MachineState {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_checked_trees::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_checked_trees::ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_checked_trees::ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_checked_trees::ProofObligationOwner::CallParameter {
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
        omega_checked_trees::ProofObligationOwner::TransitionParameter {
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

pub(crate) fn remap_invariants<'a>(
    fact_count: usize,
    facts: impl Iterator<Item = &'a omega_checked_trees::InvariantFact>,
) -> omega_core::arena::Arena<InvariantFact> {
    let mut invariants = omega_core::arena::Arena::with_capacity(fact_count);

    for fact in facts {
        invariants.append(InvariantFact {
            symbol: fact.symbol,
            name: fact.name.clone(),
            constraint_count: fact.constraint_count,
        });
    }

    invariants
}
