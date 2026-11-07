//! The invocation that completes one authored transition arm.
//!
//! This identifies argument transport, not caller-scope retirement. Entering
//! another machine still keeps caller storage and loans alive until ordinary
//! completion; only `local_jump` can establish an internal block boundary.

pub(crate) struct TerminalInvocation<'program> {
    pub(crate) target: &'program typed_trees::state::State,
    pub(crate) site: validation::CallSite<'program>,
}

pub(crate) fn terminal_invocation<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    call: &checked_trees::BorrowCallFact,
) -> Option<TerminalInvocation<'program>> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let source = program.state_in_machine(machine, state_symbol)?;
    let site = validation::find_call_site_in_state(
        program,
        machine,
        source,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arm = validation::transition_call_target(
        program,
        machine,
        source,
        call.statement_index,
        call.call_ordinal,
    )?;
    if !arm.is_valid() || !site.completes_transition(program, arm) {
        return None;
    }
    let target = program.state_by_symbol(call.target_symbol)?;
    (validation::call_target_entry_symbol(program, site.target_symbol(program)) == target.symbol)
        .then_some(TerminalInvocation { target, site })
}
