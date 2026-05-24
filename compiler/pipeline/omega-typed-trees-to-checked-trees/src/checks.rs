mod borrows;
mod contracts;

use omega_core::diagnostics::Diagnostic;

#[cfg(test)]
pub(crate) use contracts::context_proves_requirement_place_domain;

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

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
