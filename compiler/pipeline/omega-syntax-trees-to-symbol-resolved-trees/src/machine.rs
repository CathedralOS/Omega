use crate::program::Lowerer;
use crate::state::lower_state_node;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::machine::{Machine, MachineStorage, TraitConformance};
use omega_symbol_resolved_trees::state::State;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies);
    let machine_name = crate::name::lower_name(&machine.name);

    let existing_states = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .find(|existing_machine| existing_machine.name == machine_name)
        .map(|existing_machine| (existing_machine.states, existing_machine.satisfies));
    if let Some((existing_states, existing_satisfies)) = existing_states {
        let states = merge_machine_state_spans(
            &mut lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .machine_state_handles,
            existing_states,
            states,
        );
        let satisfies = merge_machine_trait_conformance_spans(
            &mut lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .machine_trait_conformances,
            existing_satisfies,
            satisfies,
        );
        let existing_machine = lowerer
            .symbol_resolved_trees
            .machines
            .find_mut(|existing_machine| existing_machine.name == machine_name)
            .expect("existing machine should still be present");
        existing_machine.states = states;
        existing_machine.satisfies = satisfies;
        return Ok(());
    }

    lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        storage: MachineStorage {
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies,
            states,
        },
    });
    Ok(())
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<TraitConformance> {
    let mut span = HandleSpan::empty();

    for trait_name in syntax_trees.items.identifier_path_members(satisfies) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .append_to_span(
                &mut span,
                TraitConformance {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(trait_name),
                },
            );
    }

    span
}

fn lower_machine_states(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<syntax::item::StateHandle>,
) -> Result<HandleSpan<Handle<State>>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for state in syntax_trees.items.state_handles(states) {
        let state = lower_state_node(lowerer, syntax_trees, syntax_trees.items.state(*state))?;
        let state = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_states
            .append(state);
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_state_handles
            .append_to_span(&mut span, state);
    }

    Ok(span)
}

fn merge_machine_state_spans(
    handles: &mut omega_core::arena::Arena<Handle<State>>,
    existing: HandleSpan<Handle<State>>,
    appended: HandleSpan<Handle<State>>,
) -> HandleSpan<Handle<State>> {
    if existing.is_empty() {
        return appended;
    }
    if appended.is_empty() {
        return existing;
    }

    let expected_next_index = existing
        .start()
        .arena_index()
        .checked_add(existing.count())
        .expect("machine state span index overflow");
    if appended.start().arena_index() == expected_next_index
        && appended.start().generation() == existing.start().generation()
    {
        return HandleSpan::from_parts(
            existing.start(),
            existing
                .count()
                .checked_add(appended.count())
                .expect("machine state span count overflow"),
        );
    }

    handles.copy_span_pair(existing, appended)
}

fn merge_machine_trait_conformance_spans(
    conformances: &mut omega_core::arena::Arena<TraitConformance>,
    existing: HandleSpan<TraitConformance>,
    appended: HandleSpan<TraitConformance>,
) -> HandleSpan<TraitConformance> {
    if existing.is_empty() {
        return appended;
    }
    if appended.is_empty() {
        return existing;
    }

    let expected_next_index = existing
        .start()
        .arena_index()
        .checked_add(existing.count())
        .expect("machine trait conformance span index overflow");
    if appended.start().arena_index() == expected_next_index
        && appended.start().generation() == existing.start().generation()
    {
        return HandleSpan::from_parts(
            existing.start(),
            existing
                .count()
                .checked_add(appended.count())
                .expect("machine trait conformance span count overflow"),
        );
    }

    conformances.copy_span_pair(existing, appended)
}
