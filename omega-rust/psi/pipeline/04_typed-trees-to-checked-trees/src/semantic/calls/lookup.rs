use crate::semantic::calls::CallSite;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;

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
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

pub(crate) fn find_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    find_state_with_machine(program, state_symbol).map(|(_, state)| state)
}

/// The machine whose state list actually stores `state_symbol`, together with
/// the state. The retained symbol parent names the owner directly when it
/// agrees with storage; the whole-program scan still runs when it does not,
/// so both tiers agree with `find_state`. Callers that need the owning
/// machine and the state — write-origin rebasing, reference-result
/// candidates, mutation summaries — take this pair instead of resolving the
/// state and then rescanning every machine for the container.
pub(crate) fn find_state_with_machine(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    if !state_symbol.is_valid() {
        return None;
    }
    let symbol = program.symbols.get(state_symbol);
    if let Some(machine) = crate::lookup::machine_by_symbol(program, symbol.parent)
        && let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    {
        return Some((machine, state));
    }
    // The whole-program scan is kept for a state whose retained parent
    // disagrees with its storage, and only for symbols that can name one.
    if !program.may_name_stored_state(symbol) {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .map(|state| (machine, state))
    })
}

/// A bare machine call names the machine head: `symbol` selects either the
/// machine itself or its entry state, never a later state. The returned pair
/// is the machine and that entry state, so machine-head callers share the
/// entry parameters, contracts, and result type without a second lookup.
/// `find_state_with_machine` alone is wider — it admits a target naming any
/// state inside the machine — so the state arm still verifies the resolved
/// state is stored first.
pub(crate) fn find_machine_head(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    if let Some(machine) = crate::lookup::machine_by_symbol(program, symbol) {
        return program
            .machine_states(machine)
            .first()
            .map(|entry| (machine, entry));
    }
    let (machine, state) = find_state_with_machine(program, symbol)?;
    program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol)
        .then_some((machine, state))
}

/// The machine whose entry state `state_symbol` names, with that state. The
/// scalar call-lowering gates spell a call's target as the entry state
/// itself: `find_machine_head` also admits the machine's own symbol, which
/// is wider than those sites resolve, so they keep this narrower lookup
/// rather than silently growing their admission.
pub(crate) fn find_machine_by_entry_state(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .first()
            .filter(|entry| entry.symbol == state_symbol)
            .map(|entry| (machine, entry))
    })
}

/// The machine `symbol` names, either directly or through its retained
/// parent: a call target or member reference may spell the machine itself or
/// one of its member declarations, and the owning machine answers for both.
/// Unlike `find_state_with_machine` this never descends into
/// `machine_states` — the symbol table's parent edge already names the
/// owner — and unlike `find_machine_head` every member resolves, not only
/// the entry state. In a well-formed program the two arms name one machine:
/// a symbol cannot be a machine and a machine's member at once.
pub(crate) fn find_machine(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !symbol.is_valid() {
        return None;
    }
    let parent = program.symbols.get(symbol).parent;
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol || machine.symbol == parent)
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
        && let Some(machine) = crate::lookup::machine_by_symbol(program, target_state_symbol)
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
    use super::SymbolHandle;
    use crate::semantic::calls::call_target_parameters;
    use crate::semantic::calls::call_target_type_parameters;
    use crate::semantic::calls::find_machine;
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
    fn machine_lookup_resolves_the_machine_or_one_of_its_members() {
        // find_machine answers the owner alone: the machine symbol names it
        // directly, and a member state's retained parent names the same
        // machine without any machine_states descent.
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let machines = symbols.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Borrowed("owner"))],
        );
        let machine_symbol = SymbolTableBuilder::child_handles(machines)
            .next()
            .expect("machine");
        let states = symbols.insert_children(
            machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Borrowed("run"))],
        );
        let state_symbol = SymbolTableBuilder::child_handles(states)
            .next()
            .expect("state");

        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..typed_trees::TypedTrees::default()
        };
        program.push_machine(Machine {
            symbol: machine_symbol,
            ..Machine::default()
        });

        for target in [machine_symbol, state_symbol] {
            assert_eq!(
                find_machine(&program, target).map(|machine| machine.symbol),
                Some(machine_symbol)
            );
        }
        assert!(find_machine(&program, SymbolHandle::invalid()).is_none());
        assert!(find_machine(&program, SymbolHandle::from_arena_index(99)).is_none());
        // A symbol whose retained parent is not a machine resolves nothing.
        assert!(find_machine(&program, root).is_none());
    }

    #[test]
    fn trait_machine_signature_is_a_call_target_outside_machine_scope() {
        // A trait requirement's own signature is a call target resolved in the
        // trait's scope: its parameters and the trait's generic context answer
        // without any machine owning the symbol.
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::name::Identifier;
        use typed_trees::signature::{StateParameter, StateSignature};
        use typed_trees::trait_definition::TraitDefinition;

        let mut program = typed_trees::TypedTrees::default();
        let trait_symbol = SymbolHandle::from_arena_index(50);
        let signature_symbol = SymbolHandle::from_arena_index(51);
        let parameter_symbol = SymbolHandle::from_arena_index(52);
        let generic_symbol = SymbolHandle::from_arena_index(53);

        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            ..TraitDefinition::default()
        };
        program.push_trait_type_parameter(
            &mut trait_definition,
            TypeParameter {
                symbol: generic_symbol,
                name: Identifier::generated("Carrier"),
                kind: TypeParameterKind::Type,
                bounds: Default::default(),
            },
        );
        let mut signature = StateSignature {
            symbol: signature_symbol,
            ..StateSignature::default()
        };
        program.push_state_signature_parameter(
            &mut signature,
            StateParameter {
                symbol: parameter_symbol,
                ..Default::default()
            },
        );
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition);

        assert_eq!(
            call_target_parameters(&program, signature_symbol).unwrap()[0].symbol,
            parameter_symbol
        );
        assert_eq!(
            call_target_type_parameters(&program, signature_symbol)[0].symbol,
            generic_symbol
        );
        // The signature's parameters answer for the signature symbol only;
        // the trait symbol itself names no call target.
        assert!(call_target_parameters(&program, trait_symbol).is_none());
        assert!(call_target_type_parameters(&program, trait_symbol).is_empty());
    }
}
