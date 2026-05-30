mod borrows;
mod capabilities;
mod contracts;
mod operators;
mod ranges;
mod termination;

use omega_core::diagnostics::Diagnostic;

pub(crate) fn check_checked_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(program, facts) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    if let Err(mut contract_diagnostics) = contracts::check_flow_call_contracts(program, facts) {
        diagnostics.append(&mut contract_diagnostics);
    }

    if let Err(mut operator_diagnostics) = operators::check_operator_resolution(facts) {
        diagnostics.append(&mut operator_diagnostics);
    }

    if let Err(mut range_diagnostics) = ranges::check_indexed_accesses(program) {
        diagnostics.append(&mut range_diagnostics);
    }

    if let Err(mut termination_diagnostics) = termination::check_machine_termination(program) {
        diagnostics.append(&mut termination_diagnostics);
    }

    if let Err(mut host_call_diagnostics) =
        capabilities::check_host_call_authorization(program, facts)
    {
        diagnostics.append(&mut host_call_diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
