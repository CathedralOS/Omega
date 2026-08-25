mod borrows;
mod carry;
mod content;
pub(crate) mod contracts;
mod crashes;
mod multiplicity;
mod operators;
mod ranges;
pub(crate) mod termination;

use psi_diagnostics::Diagnostic;

pub(crate) use multiplicity::{
    nominal_drop_machine_symbol, type_carries_linear_obligation, type_multiplicity,
};

#[cfg(test)]
pub(crate) use multiplicity::{record_permission_events, validate_linear_permission_events};

#[cfg(test)]
pub(crate) fn check_checked_facts(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    // Unit tests that assemble facts directly do not need the retained
    // permission artifact; run the same checks against a clone. The compiler
    // path below records into the owned checked tree.
    let mut scratch = facts.clone();
    check_checked_facts_recording(program, &mut scratch)
}

pub(crate) fn check_checked_facts_recording(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut psi_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let call_frames = psi_validation::CallFrameResolver::new(program);
    let incoming_guards =
        ranges::incoming_guards::IncomingGuardIndex::build(program, call_frames.as_ref());

    if let Err(mut evidence_diagnostics) = contracts::bind_call_evidence_arguments(program, facts) {
        diagnostics.append(&mut evidence_diagnostics);
    }

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(program, facts) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    if let Err(mut contract_diagnostics) =
        contracts::check_flow_call_contracts(program, facts, &incoming_guards)
    {
        diagnostics.append(&mut contract_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts, &incoming_guards)
    {
        diagnostics.append(&mut multiplicity_diagnostics);
    }

    crashes::infer_path_conditioned_guard_coverage(program, facts, &incoming_guards);
    if let Err(mut crash_diagnostics) = crashes::check_published_ceiling_coverage(program, facts) {
        diagnostics.append(&mut crash_diagnostics);
    }

    content::infer_identity_preserving_reshuffles(program, facts);
    content::compose_partition_wrappers(program, facts);

    if let Err(mut content_diagnostics) = content::check_retained_content_custody(program, facts) {
        diagnostics.append(&mut content_diagnostics);
    }

    if let Err(mut carry_diagnostics) = carry::check_suspension_carry(program, facts) {
        diagnostics.append(&mut carry_diagnostics);
    }

    if let Err(mut operator_diagnostics) = operators::check_operator_resolution(program, facts) {
        diagnostics.append(&mut operator_diagnostics);
    }

    if let Err(mut range_diagnostics) =
        ranges::check_indexed_accesses(program, call_frames.as_ref(), &incoming_guards)
    {
        diagnostics.append(&mut range_diagnostics);
    }

    if let Err(mut termination_diagnostics) = termination::check_machine_termination(program) {
        diagnostics.append(&mut termination_diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
