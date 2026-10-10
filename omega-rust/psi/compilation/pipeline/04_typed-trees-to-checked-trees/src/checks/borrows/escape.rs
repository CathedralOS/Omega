//! Normal returned views must denote storage that outlives this invocation.
//! The borrow builder captures their leaves against the current descriptor
//! roster, shared with local and store formation. An incoming referent is not
//! its owned carrier's storage; a declared lifetime alone is not an origin.
//! Locals and by-value inputs are both invocation-owned. Returning their stored
//! external references is valid; returning a fresh borrow of their storage is not.

use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) fn check_view_return_escape(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, borrow_state) in facts.borrow.states.iter() {
        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
        ) else {
            continue;
        };
        let mut reported: Vec<SymbolHandle> = Vec::new();
        for loan in facts
            .borrow
            .returned_loans
            .span_or_empty(borrow_state.returned_loans)
        {
            if loan.incoming_reference.is_valid() || reported.contains(&loan.root_symbol) {
                continue;
            }
            let root = loan.root_symbol;
            let storage = if let Some((_, local)) = program.state_local_declaration(state, root) {
                format!("the local `{}`", local.name)
            } else if let Some(parameter) =
                program.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == root
                        && !parameter.is_const
                        && validation::reference_shell(program, parameter.type_reference).is_none()
                })
            {
                format!("the owned input `{}`", parameter.name)
            } else {
                continue;
            };
            reported.push(root);
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` returns a view borrowing {storage}, which does not outlive \
                 the call; return a borrow of an input parameter or `self` instead",
                state_subject(machine, state),
            )));
        }
    }
}

pub(super) fn state_subject<'a>(
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
) -> &'a str {
    if state.name.as_str() == "entry" || machine.name.as_str().ends_with(state.name.as_str()) {
        machine.name.as_str()
    } else {
        state.name.as_str()
    }
}
