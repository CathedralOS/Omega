//! A transfer to a retained non-entry block of the current machine.
//! Calls to machine entries keep their invocation loans until completion,
//! including nested, sibling and recursive calls. Lexical ancestry and later
//! graph fusion do not authorize disposing caller storage before callee entry.
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
    // The machine roster owns actual internal blocks. Its first state is
    // the invocation entry: re-entering it is still a call, even if a later
    // recursive graph lowers that invocation to a backedge. Only a different
    // state directly in this roster can end the source block before arrival.
    let target = program.state_in_machine(machine, call.target_symbol)?;
    let entry = program.machine_states(machine).first()?;
    if target.symbol == entry.symbol {
        return None;
    }
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
