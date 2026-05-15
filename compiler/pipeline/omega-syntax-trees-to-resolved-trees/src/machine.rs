use crate::program::Lowerer;
use crate::state::lower_state_node;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::machine::{Machine, MachineStorage};
use omega_resolved_trees::state::State;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    let states = lower_machine_states(lowerer, syntax_trees, machine.states)?;
    let machine_name = crate::name::lower_name(&machine.name);

    let existing_states = lowerer
        .program
        .machines
        .iter()
        .find(|existing_machine| existing_machine.name == machine_name)
        .map(|existing_machine| existing_machine.states);
    if let Some(existing_states) = existing_states {
        let states = merge_machine_state_spans(
            &mut lowerer.program.tables.declarations.machine_state_handles,
            existing_states,
            states,
        );
        let existing_machine = lowerer
            .program
            .machines
            .find_mut(|existing_machine| existing_machine.name == machine_name)
            .expect("existing machine should still be present");
        existing_machine.states = states;
        return Ok(());
    }

    lowerer.program.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        storage: MachineStorage {
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            states,
        },
    });
    Ok(())
}

fn lower_machine_states(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<syntax::item::StateHandle>,
) -> Result<HandleSpan<Handle<State>>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for state in syntax_trees.items.state_handles(states) {
        let state = lower_state_node(lowerer, syntax_trees, syntax_trees.items.state(*state))?;
        let state = lowerer
            .program
            .tables
            .declarations
            .machine_states
            .append(state);
        let state = lowerer
            .program
            .tables
            .declarations
            .machine_state_handles
            .append(state);
        if count == 0 {
            start = state;
        }
        count = count
            .checked_add(1)
            .expect("machine state handle span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
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
