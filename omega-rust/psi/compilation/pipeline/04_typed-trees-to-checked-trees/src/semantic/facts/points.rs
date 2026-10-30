use checked_trees::{
    ContractProofFact, ContractProofFactKind, ContractProofFactOwner, ProofObligationFact,
    ProofObligationOwner,
};
use facts::{FactOrigin, ProgramPoint};
use symbols::SymbolHandle;

/// The `State` point of one machine state. Every entry row in this crate is
/// attached to a state rather than to a statement offset, so the point is
/// spelled once here instead of at each seeding site.
pub(super) fn state_point(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> ProgramPoint {
    ProgramPoint::State {
        machine_symbol,
        state_symbol,
    }
}

pub(super) fn proof_obligation_point(obligation: &ProofObligationFact) -> ProgramPoint {
    match obligation.owner {
        ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        }
        | ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => state_point(machine_symbol, state_symbol),
        ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol: _,
        } => ProgramPoint::Machine { machine_symbol },
        ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol: _,
        }
        | ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol: _,
            parameter_symbol: _,
        }
        | ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol: _,
        } => state_point(machine_symbol, state_symbol),
        ProofObligationOwner::Unknown => ProgramPoint::Global,
    }
}

pub(super) fn contract_fact_point(
    program: &dyn ::validation::CheckingInput,
    contract: &ContractProofFact,
) -> Option<ProgramPoint> {
    Some(match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            // A machine precondition is an entry assumption, not an invariant
            // to regrant at every state. Ensures keep their declaration owner
            // and become obligations at each normal exit instead.
            if contract.kind == ContractProofFactKind::Requires
                && let Some(entry) = program
                    .machine(machine_symbol)
                    .and_then(|machine| program.machine_states(machine).first())
            {
                return Some(state_point(machine_symbol, entry.symbol));
            }
            ProgramPoint::Machine { machine_symbol }
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => state_point(machine_symbol, state_symbol),
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => state_point(owner_symbol, state_symbol),
        ContractProofFactOwner::OperatorDeclaration { operator_symbol } => {
            ProgramPoint::Definition {
                symbol: operator_symbol,
            }
        }
        ContractProofFactOwner::OperatorUse {
            origin:
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    ..
                },
            ..
        } => ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        },
        ContractProofFactOwner::OperatorUse { .. } => ProgramPoint::Global,
        ContractProofFactOwner::CallSignature { .. } => return None,
        ContractProofFactOwner::Unknown => ProgramPoint::Global,
    })
}

pub(super) fn contract_fact_origin(contract: &ContractProofFact) -> Option<FactOrigin> {
    Some(match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            FactOrigin::MachineContract { machine_symbol }
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => FactOrigin::StateContract {
            machine_symbol,
            state_symbol,
        },
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => FactOrigin::StateSignatureContract {
            owner_symbol,
            state_symbol,
        },
        ContractProofFactOwner::OperatorDeclaration { operator_symbol } => match contract.kind {
            ContractProofFactKind::Requires => FactOrigin::OperatorRequires { operator_symbol },
            ContractProofFactKind::Ensures => FactOrigin::OperatorEnsures { operator_symbol },
        },
        ContractProofFactOwner::OperatorUse {
            operator_symbol, ..
        } => match contract.kind {
            ContractProofFactKind::Requires => FactOrigin::OperatorRequires { operator_symbol },
            ContractProofFactKind::Ensures => FactOrigin::OperatorEnsures { operator_symbol },
        },
        ContractProofFactOwner::CallSignature { .. } => return None,
        ContractProofFactOwner::Unknown => FactOrigin::Unknown,
    })
}
