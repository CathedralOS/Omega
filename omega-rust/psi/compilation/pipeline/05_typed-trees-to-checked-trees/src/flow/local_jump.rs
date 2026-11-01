//! One classification of a whole-invocation transfer inside a machine family.
//! Borrow admission and per-parameter read/write footprints consume this same
//! source edge. Calls in guards or operands remain ordinary returning calls;
//! lexical declaration qualifiers never introduce runtime storage.
use validation::find_state_in_machine;

pub(crate) struct LocalJump<'program> {
    pub(crate) target: &'program typed_trees::state::State,
    pub(crate) site: validation::CallSite<'program>,
}

pub(crate) fn local_jump<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    call: &checked_trees::BorrowCallFact,
) -> Option<LocalJump<'program>> {
    let source = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = program.machine_by_symbol(machine_symbol)?;
    // The same transfer reaches a sibling state of this machine or the head
    // of a machine inside its enclosing-machine family; both leave the source
    // scope when the transition's invocation completes. A machine head outside
    // that family stays an ordinary call.
    let target =
        find_state_in_machine(program, machine_symbol, call.target_symbol).or_else(|| {
            validation::find_machine_head(program, call.target_symbol)
                .filter(|(destination, _)| {
                    machine_in_frame_family(program, machine_symbol, destination.symbol)
                })
                .map(|(_, entry)| entry)
        })?;
    let site = validation::find_call_site_in_state(
        program,
        machine,
        source,
        call.statement_index,
        call.call_ordinal,
    )?;
    match &site {
        validation::CallSite::TransitionNamed { path, .. } => {
            // A local jump name has no explicit receiver; its head resolves
            // to the entered body, not a borrowed receiver alias. A machine
            // head's name reaches that body through its entry state.
            (path.members.count() == 1
                && validation::call_target_entry_symbol(program, path.head_symbol) == target.symbol)
                .then_some(LocalJump { target, site })
        }
        validation::CallSite::Expression { call: invoked, .. } => {
            // The same edge lowered as the arm's call operand. Only the
            // arm's whole invocation completes it; calls inside its guard
            // or operands evaluate in the source state and stay ordinary
            // calls. A declaration qualifier carries no runtime receiver.
            let completes = validation::transition_call_target(
                program,
                machine,
                source,
                call.statement_index,
                call.call_ordinal,
            )
            .is_some_and(|arm| arm.is_valid() && site.completes_transition(program, arm));
            (completes
                && program.call_has_no_runtime_receiver_for_target(invoked)
                && validation::call_target_entry_symbol(program, site.target_symbol(program))
                    == target.symbol)
                .then_some(LocalJump { target, site })
        }
        _ => None,
    }
}

/// Whether `callee` is the caller's machine, one of its enclosing machine
/// ancestors, or a machine declared directly inside one of them. Sibling and
/// nested machines carry the same transition-transfer semantics their states
/// did before the nested-machine migration; an ancestor's head is re-entered
/// as the same family's own transfer.
fn machine_in_frame_family(
    program: &dyn ::validation::CheckingInput,
    caller: symbols::SymbolHandle,
    callee: symbols::SymbolHandle,
) -> bool {
    let mut scope = caller;
    loop {
        if callee == scope || program.symbols.get(callee).parent == scope {
            return true;
        }
        let parent = program.symbols.get(scope).parent;
        if !parent.is_valid() || program.symbols.get(parent).kind != symbols::SymbolKind::Machine {
            return false;
        }
        scope = parent;
    }
}
