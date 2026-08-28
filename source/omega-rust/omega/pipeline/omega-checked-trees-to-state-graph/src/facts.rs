use omega_state_graph::{ProofFactKind, ProofObligationFact, ProofObligationOwner};
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn remap_proof_obligations(
    program: &CheckedTrees,
) -> Result<psi_arena::Arena<ProofObligationFact>, Diagnostic> {
    let facts = &program.facts.proof.obligations;
    let mut obligations = psi_arena::Arena::with_capacity(facts.len());

    for (_, fact) in facts.iter() {
        validate_proof_owner(program, fact)?;
        obligations.append(ProofObligationFact {
            kind: match fact.kind {
                psi_checked_trees::ProofFactKind::BoundedAssignment => {
                    ProofFactKind::BoundedAssignment
                }
                psi_checked_trees::ProofFactKind::BoundedCallArgument => {
                    ProofFactKind::BoundedCallArgument
                }
                psi_checked_trees::ProofFactKind::BoundedInitializer => {
                    ProofFactKind::BoundedInitializer
                }
                psi_checked_trees::ProofFactKind::BoundedStateReturn => {
                    ProofFactKind::BoundedStateReturn
                }
                psi_checked_trees::ProofFactKind::BoundedValue => ProofFactKind::BoundedValue,
                psi_checked_trees::ProofFactKind::BoundedTransitionArgument => {
                    ProofFactKind::BoundedTransitionArgument
                }
                psi_checked_trees::ProofFactKind::GuardedTransition => {
                    ProofFactKind::GuardedTransition
                }
            },
            machine_symbol: fact.machine_symbol,
            state_symbol: fact.state_symbol,
            owner: remap_proof_owner(&fact.owner),
        });
    }

    Ok(obligations)
}

fn validate_proof_owner(
    program: &CheckedTrees,
    fact: &psi_checked_trees::ProofObligationFact,
) -> Result<(), Diagnostic> {
    use psi_checked_trees::ProofFactKind as Kind;
    use psi_checked_trees::ProofObligationOwner as Owner;

    match &fact.owner {
        Owner::Unknown => Err(Diagnostic::error(
            "state-graph proof obligation has unknown owner authority",
        )),
        Owner::MachineState {
            machine_symbol,
            state_symbol,
        } => {
            require_kind(
                matches!(
                    &fact.kind,
                    Kind::BoundedAssignment | Kind::GuardedTransition
                ),
                "machine-state",
            )?;
            require_outer_state(fact, *machine_symbol, *state_symbol)?;
            exact_state(program, *machine_symbol, *state_symbol)?;
            Ok(())
        }
        Owner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => {
            require_kind(
                matches!(&fact.kind, Kind::BoundedInitializer | Kind::BoundedValue),
                "machine-owned-data",
            )?;
            require_empty_outer_state(fact)?;
            exact_owned_data(program, *machine_symbol, *data_symbol)?;
            Ok(())
        }
        Owner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => {
            require_kind(matches!(&fact.kind, Kind::BoundedValue), "state-parameter")?;
            require_empty_outer_state(fact)?;
            exact_state_parameter(program, *machine_symbol, *state_symbol, *parameter_symbol)?;
            Ok(())
        }
        Owner::StateReturn {
            machine_symbol,
            state_symbol,
        } => {
            require_kind(
                matches!(&fact.kind, Kind::BoundedStateReturn | Kind::BoundedValue),
                "state-return",
            )?;
            if matches!(&fact.kind, Kind::BoundedStateReturn) {
                require_outer_state(fact, *machine_symbol, *state_symbol)?;
            } else {
                require_empty_outer_state(fact)?;
            }
            let state = exact_state(program, *machine_symbol, *state_symbol)?;
            if !state.return_type.is_valid() {
                return Err(Diagnostic::error(
                    "state-graph proof state-return owner has no retained return type",
                ));
            }
            Ok(())
        }
        Owner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol,
            parameter_symbol,
        } => {
            require_kind(
                matches!(&fact.kind, Kind::BoundedCallArgument),
                "call-parameter",
            )?;
            require_outer_state(fact, *machine_symbol, *state_symbol)?;
            exact_state(program, *machine_symbol, *state_symbol)?;
            exact_callable_parameter(program, *target_symbol, *parameter_symbol)
        }
        Owner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => {
            require_kind(
                matches!(&fact.kind, Kind::BoundedTransitionArgument),
                "transition-parameter",
            )?;
            require_outer_state(fact, *machine_symbol, *state_symbol)?;
            exact_state(program, *machine_symbol, *state_symbol)?;
            exact_global_state_parameter(program, *parameter_symbol)
        }
    }
}

fn require_kind(accepted: bool, owner: &str) -> Result<(), Diagnostic> {
    if accepted {
        Ok(())
    } else {
        Err(Diagnostic::error(format!(
            "state-graph proof {owner} owner is incompatible with its fact kind"
        )))
    }
}

fn require_outer_state(
    fact: &psi_checked_trees::ProofObligationFact,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Result<(), Diagnostic> {
    if fact.machine_symbol == machine && fact.state_symbol == state {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "state-graph proof outer machine/state coordinate disagrees with its owner",
        ))
    }
}

fn require_empty_outer_state(
    fact: &psi_checked_trees::ProofObligationFact,
) -> Result<(), Diagnostic> {
    if !fact.machine_symbol.is_valid() && !fact.state_symbol.is_valid() {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "state-graph owner-only proof fact retains an unexpected outer machine/state coordinate",
        ))
    }
}

fn exact_machine<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::machine::Machine, Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph proof machine owner identity is invalid",
        ));
    }
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches
        .next()
        .ok_or_else(|| Diagnostic::error("state-graph proof machine owner is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "state-graph proof machine owner is duplicated",
        ));
    }
    Ok(machine)
}

fn exact_state<'program>(
    program: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::state::State, Diagnostic> {
    if !state_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph proof state owner identity is invalid",
        ));
    }
    let machine = exact_machine(program, machine_symbol)?;
    let mut matches = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == state_symbol);
    let state = matches
        .next()
        .ok_or_else(|| Diagnostic::error("state-graph proof state is missing from its owner"))?;
    if matches.next().is_some()
        || program.machines().iter().any(|candidate_machine| {
            candidate_machine.symbol != machine_symbol
                && program
                    .machine_states(candidate_machine)
                    .iter()
                    .any(|candidate| candidate.symbol == state_symbol)
        })
    {
        return Err(Diagnostic::error(
            "state-graph proof state owner coordinate is duplicated or cross-owned",
        ));
    }
    Ok(state)
}

fn exact_owned_data(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    data_symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    if !data_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph proof owned-data identity is invalid",
        ));
    }
    let machine = exact_machine(program, machine_symbol)?;
    let owner_matches = program
        .machine_owned_data(machine)
        .iter()
        .filter(|data| data.symbol == data_symbol)
        .count();
    let all_matches = program
        .machines()
        .iter()
        .flat_map(|candidate| program.machine_owned_data(candidate))
        .filter(|data| data.symbol == data_symbol)
        .count();
    if owner_matches == 1 && all_matches == 1 {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "state-graph proof owned-data coordinate is missing, duplicated, or cross-owned",
        ))
    }
}

fn exact_state_parameter(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    if !parameter_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph proof state-parameter identity is invalid",
        ));
    }
    let state = exact_state(program, machine_symbol, state_symbol)?;
    let owner_matches = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == parameter_symbol)
        .count();
    let all_matches = global_state_parameter_count(program, parameter_symbol, true);
    if owner_matches == 1 && all_matches == 1 {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "state-graph proof state-parameter coordinate is missing, duplicated, or cross-owned",
        ))
    }
}

fn exact_global_state_parameter(
    program: &CheckedTrees,
    parameter_symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    if parameter_symbol.is_valid()
        && global_state_parameter_count(program, parameter_symbol, false) == 1
    {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "state-graph proof transition parameter is missing or ambiguous",
        ))
    }
}

fn global_state_parameter_count(
    program: &CheckedTrees,
    symbol: SymbolHandle,
    include_self: bool,
) -> usize {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.state_parameters(state))
        .filter(|parameter| (include_self || !parameter.is_self) && parameter.symbol == symbol)
        .count()
}

fn exact_callable_parameter(
    program: &CheckedTrees,
    target_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    if !target_symbol.is_valid() || !parameter_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph proof call target or parameter identity is invalid",
        ));
    }
    let mut target_count = 0usize;
    let mut parameter_count = 0usize;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if state.symbol == target_symbol {
                target_count += 1;
                parameter_count += program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| !parameter.is_self && parameter.symbol == parameter_symbol)
                    .count();
            }
        }
        for type_parameter in program.machine_type_parameters(machine) {
            let psi_checked_trees::data::TypeParameterKind::Machine { contract } =
                &type_parameter.kind
            else {
                continue;
            };
            if type_parameter.symbol == target_symbol {
                let contract = program
                    .machine_parameter_contract_view(contract)
                    .expect(
                        "checked machine-parameter contract must retain a valid requirement identity",
                    )
                    .signature();
                target_count += 1;
                parameter_count += program
                    .state_signature_parameters(contract)
                    .iter()
                    .filter(|parameter| !parameter.is_self && parameter.symbol == parameter_symbol)
                    .count();
            }
        }
    }
    if target_count != 1 {
        return Err(Diagnostic::error(
            "state-graph proof call target is missing or ambiguous across callable categories",
        ));
    }
    if parameter_count != 1 {
        return Err(Diagnostic::error(
            "state-graph proof call parameter is missing or duplicated in its exact target",
        ));
    }
    Ok(())
}

fn remap_proof_owner(owner: &psi_checked_trees::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        psi_checked_trees::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        psi_checked_trees::ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::MachineState {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        psi_checked_trees::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        psi_checked_trees::ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        psi_checked_trees::ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        psi_checked_trees::ProofObligationOwner::CallParameter {
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
        psi_checked_trees::ProofObligationOwner::TransitionParameter {
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::data::{TypeParameter, TypeParameterKind};
    use psi_checked_trees::machine::{Machine, OwnedData};
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::signature::{StateParameter, StateSignature};
    use psi_checked_trees::state::State;
    use psi_checked_trees::types::TypeReferenceNode;

    const MACHINE: u32 = 1;
    const SOURCE_STATE: u32 = 2;
    const TARGET_STATE: u32 = 3;
    const SOURCE_PARAMETER: u32 = 4;
    const TARGET_PARAMETER: u32 = 5;
    const OWNED_DATA: u32 = 6;
    const GENERIC_TARGET: u32 = 7;
    const GENERIC_CONTRACT: u32 = 8;
    const GENERIC_PARAMETER: u32 = 9;

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn parameter(index: u32, name: &str) -> StateParameter {
        StateParameter {
            symbol: symbol(index),
            name: Identifier::generated(name),
            ..Default::default()
        }
    }

    fn fixture() -> CheckedTrees {
        let mut program = CheckedTrees::default();
        let unit = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Unit);
        let mut machine = Machine {
            symbol: symbol(MACHINE),
            name: Identifier::generated("Root"),
            ..Default::default()
        };

        let mut generic_contract = StateSignature {
            symbol: symbol(GENERIC_CONTRACT),
            name: Identifier::generated("GenericTarget"),
            ..Default::default()
        };
        program.typed.push_state_signature_parameter(
            &mut generic_contract,
            parameter(GENERIC_PARAMETER, "generic_value"),
        );
        program.typed.push_machine_type_parameter(
            &mut machine,
            TypeParameter {
                symbol: symbol(GENERIC_TARGET),
                name: Identifier::generated("Target"),
                kind: TypeParameterKind::Machine {
                    contract: psi_checked_trees::data::MachineParameterContract::Structural(
                        generic_contract,
                    ),
                },
                ..Default::default()
            },
        );
        program.typed.push_machine_owned_data(
            &mut machine,
            OwnedData {
                symbol: symbol(OWNED_DATA),
                name: Identifier::generated("stored"),
                type_reference: unit,
                initial_value: Default::default(),
            },
        );

        let mut source = State {
            symbol: symbol(SOURCE_STATE),
            name: Identifier::generated("source"),
            return_type: unit,
            ..Default::default()
        };
        program
            .typed
            .push_state_parameter(&mut source, parameter(SOURCE_PARAMETER, "source_value"));
        program.typed.push_machine_state(&mut machine, source);

        let mut target = State {
            symbol: symbol(TARGET_STATE),
            name: Identifier::generated("target"),
            ..Default::default()
        };
        program
            .typed
            .push_state_parameter(&mut target, parameter(TARGET_PARAMETER, "target_value"));
        program.typed.push_machine_state(&mut machine, target);
        program.typed.push_machine(machine);
        program
    }

    fn state_owner() -> psi_checked_trees::ProofObligationOwner {
        psi_checked_trees::ProofObligationOwner::MachineState {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(SOURCE_STATE),
        }
    }

    fn fact(
        kind: psi_checked_trees::ProofFactKind,
        owner: psi_checked_trees::ProofObligationOwner,
    ) -> psi_checked_trees::ProofObligationFact {
        let (machine_symbol, state_symbol) = match (&kind, &owner) {
            (
                psi_checked_trees::ProofFactKind::BoundedAssignment
                | psi_checked_trees::ProofFactKind::BoundedCallArgument
                | psi_checked_trees::ProofFactKind::BoundedStateReturn
                | psi_checked_trees::ProofFactKind::BoundedTransitionArgument
                | psi_checked_trees::ProofFactKind::GuardedTransition,
                psi_checked_trees::ProofObligationOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                }
                | psi_checked_trees::ProofObligationOwner::StateReturn {
                    machine_symbol,
                    state_symbol,
                }
                | psi_checked_trees::ProofObligationOwner::CallParameter {
                    machine_symbol,
                    state_symbol,
                    ..
                }
                | psi_checked_trees::ProofObligationOwner::TransitionParameter {
                    machine_symbol,
                    state_symbol,
                    ..
                },
            ) => (*machine_symbol, *state_symbol),
            _ => (SymbolHandle::invalid(), SymbolHandle::invalid()),
        };
        psi_checked_trees::ProofObligationFact {
            kind,
            machine_symbol,
            state_symbol,
            owner,
        }
    }

    fn error(program: &CheckedTrees, fact: &psi_checked_trees::ProofObligationFact) -> String {
        validate_proof_owner(program, fact)
            .expect_err("invalid proof owner must fail closed")
            .message
    }

    fn add_foreign_state(
        program: &mut CheckedTrees,
        machine_symbol: u32,
        state_symbol: u32,
        parameter_symbol: u32,
    ) {
        let mut machine = Machine {
            symbol: symbol(machine_symbol),
            name: Identifier::generated("Foreign"),
            ..Default::default()
        };
        let mut state = State {
            symbol: symbol(state_symbol),
            name: Identifier::generated("foreign"),
            ..Default::default()
        };
        program
            .typed
            .push_state_parameter(&mut state, parameter(parameter_symbol, "foreign_value"));
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);
    }

    #[test]
    fn exact_owner_categories_copy_opaquely_in_carrier_order() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let mut program = fixture();
        let facts = [
            fact(Kind::BoundedAssignment, state_owner()),
            fact(Kind::GuardedTransition, state_owner()),
            fact(
                Kind::BoundedInitializer,
                Owner::MachineOwnedData {
                    machine_symbol: symbol(MACHINE),
                    data_symbol: symbol(OWNED_DATA),
                },
            ),
            fact(
                Kind::BoundedValue,
                Owner::MachineOwnedData {
                    machine_symbol: symbol(MACHINE),
                    data_symbol: symbol(OWNED_DATA),
                },
            ),
            fact(
                Kind::BoundedValue,
                Owner::StateParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    parameter_symbol: symbol(SOURCE_PARAMETER),
                },
            ),
            fact(
                Kind::BoundedValue,
                Owner::StateReturn {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                },
            ),
            fact(
                Kind::BoundedStateReturn,
                Owner::StateReturn {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                },
            ),
            fact(
                Kind::BoundedCallArgument,
                Owner::CallParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    target_symbol: symbol(TARGET_STATE),
                    parameter_symbol: symbol(TARGET_PARAMETER),
                },
            ),
            fact(
                Kind::BoundedCallArgument,
                Owner::CallParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    target_symbol: symbol(GENERIC_TARGET),
                    parameter_symbol: symbol(GENERIC_PARAMETER),
                },
            ),
            fact(
                Kind::BoundedTransitionArgument,
                Owner::TransitionParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    parameter_symbol: symbol(TARGET_PARAMETER),
                },
            ),
        ];
        for fact in facts.clone() {
            program.facts.proof.obligations.append(fact);
        }

        let copied = remap_proof_obligations(&program).expect("exact proof owners");
        assert_eq!(copied.len(), facts.len());
        for ((_, copied), source) in copied.iter().zip(facts.iter()) {
            assert_eq!(copied.machine_symbol, source.machine_symbol);
            assert_eq!(copied.state_symbol, source.state_symbol);
        }
        assert!(matches!(
            copied.iter().last().map(|(_, fact)| &fact.owner),
            Some(ProofObligationOwner::TransitionParameter { parameter_symbol, .. })
                if *parameter_symbol == symbol(TARGET_PARAMETER)
        ));
    }

    #[test]
    fn unknown_kind_and_outer_coordinate_drift_fail_closed() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let program = fixture();
        let unknown = fact(Kind::BoundedValue, Owner::Unknown);
        assert!(error(&program, &unknown).contains("unknown owner"));

        let wrong_kind = fact(
            Kind::BoundedInitializer,
            Owner::MachineState {
                machine_symbol: symbol(MACHINE),
                state_symbol: symbol(SOURCE_STATE),
            },
        );
        assert!(error(&program, &wrong_kind).contains("incompatible"));

        let mut outer_drift = fact(Kind::BoundedAssignment, state_owner());
        outer_drift.state_symbol = symbol(TARGET_STATE);
        assert!(error(&program, &outer_drift).contains("outer machine/state"));

        let mut unexpected_outer = fact(
            Kind::BoundedValue,
            Owner::MachineOwnedData {
                machine_symbol: symbol(MACHINE),
                data_symbol: symbol(OWNED_DATA),
            },
        );
        unexpected_outer.machine_symbol = symbol(MACHINE);
        assert!(error(&program, &unexpected_outer).contains("unexpected outer"));
    }

    #[test]
    fn machine_and_state_owners_reject_missing_duplicate_and_cross_owned_rows() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let missing_machine = fact(
            Kind::BoundedAssignment,
            Owner::MachineState {
                machine_symbol: symbol(90),
                state_symbol: symbol(SOURCE_STATE),
            },
        );
        assert!(error(&fixture(), &missing_machine).contains("machine owner is missing"));

        let mut duplicate_machine = fixture();
        let machine = duplicate_machine.machines()[0].clone();
        duplicate_machine.typed.push_machine(machine);
        assert!(
            error(
                &duplicate_machine,
                &fact(Kind::BoundedAssignment, state_owner())
            )
            .contains("machine owner is duplicated")
        );

        let mut cross_owned = fixture();
        add_foreign_state(&mut cross_owned, 20, SOURCE_STATE, 21);
        assert!(
            error(&cross_owned, &fact(Kind::BoundedAssignment, state_owner()))
                .contains("duplicated or cross-owned")
        );

        let missing_state = fact(
            Kind::BoundedAssignment,
            Owner::MachineState {
                machine_symbol: symbol(MACHINE),
                state_symbol: symbol(99),
            },
        );
        assert!(error(&fixture(), &missing_state).contains("missing from its owner"));
    }

    #[test]
    fn data_and_state_parameter_owners_must_resolve_inside_one_exact_owner() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let missing_data = fact(
            Kind::BoundedValue,
            Owner::MachineOwnedData {
                machine_symbol: symbol(MACHINE),
                data_symbol: symbol(91),
            },
        );
        assert!(error(&fixture(), &missing_data).contains("owned-data coordinate"));

        let mut duplicate_data = fixture();
        let mut owner = duplicate_data.machines()[0].clone();
        let unit = duplicate_data
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Unit);
        duplicate_data.typed.push_machine_owned_data(
            &mut owner,
            OwnedData {
                symbol: symbol(OWNED_DATA),
                name: Identifier::generated("duplicate"),
                type_reference: unit,
                initial_value: Default::default(),
            },
        );
        duplicate_data.typed.machines_mut()[0] = owner;
        assert!(
            error(
                &duplicate_data,
                &fact(
                    Kind::BoundedValue,
                    Owner::MachineOwnedData {
                        machine_symbol: symbol(MACHINE),
                        data_symbol: symbol(OWNED_DATA),
                    },
                )
            )
            .contains("owned-data coordinate")
        );

        let missing_parameter = fact(
            Kind::BoundedValue,
            Owner::StateParameter {
                machine_symbol: symbol(MACHINE),
                state_symbol: symbol(SOURCE_STATE),
                parameter_symbol: symbol(92),
            },
        );
        assert!(error(&fixture(), &missing_parameter).contains("state-parameter coordinate"));

        let mut duplicate_parameter = fixture();
        let owner = duplicate_parameter.machines()[0].clone();
        let mut target = duplicate_parameter.machine_states(&owner)[1].clone();
        duplicate_parameter
            .typed
            .push_state_parameter(&mut target, parameter(TARGET_PARAMETER, "duplicate"));
        duplicate_parameter.typed.machine_states_mut(&owner)[1] = target;
        let target_parameter = fact(
            Kind::BoundedValue,
            Owner::StateParameter {
                machine_symbol: symbol(MACHINE),
                state_symbol: symbol(TARGET_STATE),
                parameter_symbol: symbol(TARGET_PARAMETER),
            },
        );
        assert!(
            error(&duplicate_parameter, &target_parameter).contains("state-parameter coordinate")
        );

        let mut cross_parameter = fixture();
        add_foreign_state(&mut cross_parameter, 30, 31, SOURCE_PARAMETER);
        let source_parameter = fact(
            Kind::BoundedValue,
            Owner::StateParameter {
                machine_symbol: symbol(MACHINE),
                state_symbol: symbol(SOURCE_STATE),
                parameter_symbol: symbol(SOURCE_PARAMETER),
            },
        );
        assert!(error(&cross_parameter, &source_parameter).contains("cross-owned"));
    }

    #[test]
    fn call_parameter_requires_one_exact_callable_category_and_owned_parameter() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let call = |target_symbol, parameter_symbol| {
            fact(
                Kind::BoundedCallArgument,
                Owner::CallParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    target_symbol,
                    parameter_symbol,
                },
            )
        };
        assert!(
            error(&fixture(), &call(symbol(90), symbol(TARGET_PARAMETER)))
                .contains("target is missing or ambiguous")
        );
        assert!(
            error(
                &fixture(),
                &call(symbol(TARGET_STATE), symbol(SOURCE_PARAMETER))
            )
            .contains("parameter is missing or duplicated")
        );

        let mut ambiguous = fixture();
        add_foreign_state(&mut ambiguous, 40, TARGET_STATE, 41);
        assert!(
            error(
                &ambiguous,
                &call(symbol(TARGET_STATE), symbol(TARGET_PARAMETER))
            )
            .contains("target is missing or ambiguous")
        );
    }

    #[test]
    fn transition_parameter_requires_one_unique_retained_state_parameter() {
        use psi_checked_trees::ProofFactKind as Kind;
        use psi_checked_trees::ProofObligationOwner as Owner;

        let transition = |parameter_symbol| {
            fact(
                Kind::BoundedTransitionArgument,
                Owner::TransitionParameter {
                    machine_symbol: symbol(MACHINE),
                    state_symbol: symbol(SOURCE_STATE),
                    parameter_symbol,
                },
            )
        };
        assert!(error(&fixture(), &transition(symbol(90))).contains("missing or ambiguous"));

        let mut ambiguous = fixture();
        add_foreign_state(&mut ambiguous, 50, 51, TARGET_PARAMETER);
        assert!(
            error(&ambiguous, &transition(symbol(TARGET_PARAMETER)))
                .contains("missing or ambiguous")
        );
    }
}
