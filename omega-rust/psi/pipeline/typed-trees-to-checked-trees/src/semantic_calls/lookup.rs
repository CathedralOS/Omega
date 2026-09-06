use super::*;

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression { call, .. } => {
            program.expression_table.expression_handles(call.arguments)
        }
        CallSite::TransitionNamed { arguments, .. } => {
            program.statement_table.expression_handles(*arguments)
        }
    }
}

pub(crate) fn find_state_in_machine(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

pub(crate) fn find_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    if !state_symbol.is_valid() {
        return None;
    }
    let machine_symbol = program.symbols.get(state_symbol).parent;
    find_state_in_machine(program, machine_symbol, state_symbol).or_else(|| {
        program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
        })
    })
}

/// The parameter list of a call target: a machine entry or selected state's
/// parameters, or --
/// for a call through a trait-typed receiver (boundary trait machines) or a
/// boundary-trait receiver -- the owning signature's parameters.
pub(crate) fn call_target_parameters(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    if target_state_symbol.is_valid()
        && let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == target_state_symbol)
    {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| program.state_parameters(entry));
    }
    if let Some(state) = find_state(program, target_state_symbol) {
        return Some(program.state_parameters(state));
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(program.state_signature_parameters(signature));
    }

    program
        .traits()
        .iter()
        .find_map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.symbol == target_state_symbol)
        })
        .map(|signature| program.state_signature_parameters(signature))
}

/// The generic declaration context that owns a call target's formal
/// parameters. Carry and other structural properties must interpret `T`
/// against these bounds, not against an unrelated caller's same-spelled type
/// parameter and not as an invented concrete type.
pub(crate) fn call_target_type_parameters(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> &[typed_trees::data::TypeParameter] {
    if let Some(machine) = program.machines().iter().find(|machine| {
        target_state_symbol.is_valid()
            && (machine.symbol == target_state_symbol
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == target_state_symbol))
    }) {
        return program.machine_type_parameters(machine);
    }

    if let Some((machine, _)) = program.machine_parameter_signature(target_state_symbol) {
        return program.machine_type_parameters(machine);
    }

    if let Some(trait_definition) = program.traits().iter().find(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.symbol == target_state_symbol)
    }) {
        return program.trait_type_parameters(trait_definition);
    }

    &[]
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{machine::Machine, state::State};

    #[test]
    fn machine_head_calls_use_entry_parameters_and_the_same_generic_context() {
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::name::Identifier;
        use typed_trees::signature::StateParameter;

        let mut program = typed_trees::TypedTrees::default();
        let machine_symbol = SymbolHandle::from_arena_index(40);
        let entry_symbol = SymbolHandle::from_arena_index(41);
        let later_symbol = SymbolHandle::from_arena_index(42);
        let parameter_symbol = SymbolHandle::from_arena_index(43);
        let generic_symbol = SymbolHandle::from_arena_index(44);
        let later_parameter = SymbolHandle::from_arena_index(45);
        let mut machine = Machine {
            symbol: machine_symbol,
            ..Default::default()
        };
        program.push_machine_type_parameter(
            &mut machine,
            TypeParameter {
                symbol: generic_symbol,
                name: Identifier::generated("Value"),
                kind: TypeParameterKind::Type,
                bounds: Default::default(),
            },
        );
        for (state_symbol, parameter) in [
            (entry_symbol, parameter_symbol),
            (later_symbol, later_parameter),
        ] {
            let mut state = State {
                symbol: state_symbol,
                ..Default::default()
            };
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol: parameter,
                    ..Default::default()
                },
            );
            program.push_machine_state(&mut machine, state);
        }
        program.push_machine(machine);
        for target in [machine_symbol, entry_symbol] {
            assert_eq!(
                call_target_parameters(&program, target).unwrap()[0].symbol,
                parameter_symbol
            );
            assert_eq!(
                call_target_type_parameters(&program, target)[0].symbol,
                generic_symbol
            );
        }
        assert_eq!(
            call_target_parameters(&program, later_symbol).unwrap()[0].symbol,
            later_parameter
        );
        assert_eq!(
            call_target_type_parameters(&program, later_symbol)[0].symbol,
            generic_symbol
        );
        assert!(call_target_parameters(&program, SymbolHandle::invalid()).is_none());
        assert!(call_target_type_parameters(&program, SymbolHandle::invalid()).is_empty());
        assert!(call_target_parameters(&program, SymbolHandle::from_arena_index(99)).is_none());
    }

    #[test]
    fn state_lookup_rejects_invalid_handles_but_retains_unresolved_table_fallback() {
        let mut program = typed_trees::TypedTrees::default();
        assert!(find_state(&program, SymbolHandle::invalid()).is_none());

        let machine_symbol = SymbolHandle::from_arena_index(40);
        let state_symbol = SymbolHandle::from_arena_index(41);
        let mut machine = Machine {
            symbol: machine_symbol,
            ..Machine::default()
        };
        program.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                ..State::default()
            },
        );
        program.push_machine(machine);

        assert_eq!(
            find_state(&program, state_symbol).map(|state| state.symbol),
            Some(state_symbol)
        );
    }

    #[test]
    fn state_lookup_falls_back_when_retained_parent_disagrees_with_storage() {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let machines = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Borrowed("stored")),
                (SymbolKind::Machine, SymbolNameRef::Borrowed("retained")),
            ],
        );
        let mut machine_symbols = SymbolTableBuilder::child_handles(machines);
        let stored_machine_symbol = machine_symbols.next().expect("stored machine");
        let retained_machine_symbol = machine_symbols.next().expect("retained machine");
        let states = symbols.insert_children(
            retained_machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Borrowed("run"))],
        );
        let state_symbol = SymbolTableBuilder::child_handles(states)
            .next()
            .expect("state");

        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..typed_trees::TypedTrees::default()
        };
        let mut stored_machine = Machine {
            symbol: stored_machine_symbol,
            ..Machine::default()
        };
        program.push_machine_state(
            &mut stored_machine,
            State {
                symbol: state_symbol,
                ..State::default()
            },
        );
        program.push_machine(stored_machine);
        program.push_machine(Machine {
            symbol: retained_machine_symbol,
            ..Machine::default()
        });

        assert_eq!(
            find_state(&program, state_symbol).map(|state| state.symbol),
            Some(state_symbol)
        );
    }
}
