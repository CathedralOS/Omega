mod borrows;
mod capabilities;
mod carry;
mod contracts;
mod multiplicity;
mod operators;
mod ranges;
pub(crate) mod termination;

use omega_core::diagnostics::Diagnostic;

pub(crate) use multiplicity::{type_carries_linear_obligation, type_multiplicity};

#[cfg(test)]
pub(crate) use multiplicity::{record_permission_events, validate_linear_permission_events};

#[cfg(test)]
pub(crate) fn check_checked_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    // Unit tests that assemble facts directly do not need the retained
    // permission artifact; run the same checks against a clone. The compiler
    // path below records into the owned checked tree.
    let mut scratch = facts.clone();
    check_checked_facts_recording(program, &mut scratch)
}

pub(crate) fn check_checked_facts_recording(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut omega_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(program, facts) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    if let Err(mut contract_diagnostics) = contracts::check_flow_call_contracts(program, facts) {
        diagnostics.append(&mut contract_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts)
    {
        diagnostics.append(&mut multiplicity_diagnostics);
    }

    if let Err(mut carry_diagnostics) = carry::check_suspension_carry(program, facts) {
        diagnostics.append(&mut carry_diagnostics);
    }

    if let Err(mut operator_diagnostics) = operators::check_operator_resolution(program, facts) {
        diagnostics.append(&mut operator_diagnostics);
    }

    if let Err(mut range_diagnostics) = ranges::check_indexed_accesses(program) {
        diagnostics.append(&mut range_diagnostics);
    }

    if let Err(mut termination_diagnostics) = termination::check_machine_termination(program) {
        diagnostics.append(&mut termination_diagnostics);
    }

    if let Err(mut host_call_diagnostics) =
        capabilities::check_boundary_provider_approval(program, facts)
    {
        diagnostics.append(&mut host_call_diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
