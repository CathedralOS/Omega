use super::*;

pub(super) fn proof_obligation_point(obligation: &ProofObligationFact) -> ProgramPoint {
    match obligation.owner {
        ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        }
        | ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
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
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        ProofObligationOwner::Unknown => ProgramPoint::Global,
    }
}

pub(super) fn contract_fact_point(
    program: &typed_trees::TypedTrees,
    contract: &ContractProofFact,
) -> ProgramPoint {
    match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            // A machine precondition is an entry assumption, not an invariant
            // to regrant at every state. Ensures keep their declaration owner
            // and become obligations at each normal exit instead.
            if contract.kind == ContractProofFactKind::Requires
                && let Some(entry) = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == machine_symbol)
                    .and_then(|machine| program.machine_states(machine).first())
            {
                return ProgramPoint::State {
                    machine_symbol,
                    state_symbol: entry.symbol,
                };
            }
            ProgramPoint::Machine { machine_symbol }
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol: owner_symbol,
            state_symbol,
        },
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
        ContractProofFactOwner::Unknown => ProgramPoint::Global,
    }
}

pub(super) fn contract_fact_origin(contract: &ContractProofFact) -> FactOrigin {
    match contract.owner {
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
        ContractProofFactOwner::Unknown => FactOrigin::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_contract_origin_keeps_contract_kind_and_operator_symbol() {
        let operator_symbol = SymbolHandle::from_arena_index(8);
        let contract = ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::OperatorUse {
                expression: ExpressionHandle::from_arena_index(1),
                origin: Default::default(),
                operator_symbol,
            },
            fact: Handle::from_arena_index(2),
            evidence_term: None,
            qualification_authorization: None,
        };

        assert_eq!(
            contract_fact_origin(&contract),
            FactOrigin::OperatorRequires { operator_symbol }
        );
    }
}
