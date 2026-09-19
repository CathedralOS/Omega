mod borrows;
mod carry;
mod content;
pub(crate) mod contracts;
mod crashes;
mod multiplicity;
mod operators;
mod ranges;
pub(crate) mod termination;

use diagnostics::Diagnostic;
pub(crate) use operators::{named_operator_route_is_false, operator_route_is_false};

pub(crate) use multiplicity::{
    nominal_drop_machine_symbol, type_carries_linear_obligation, type_multiplicity,
};

#[cfg(test)]
pub(crate) use multiplicity::{record_permission_events, validate_linear_permission_events};

#[cfg(test)]
pub(crate) fn check_checked_facts(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut scratch = facts.clone();
    check_checked_facts_recording(program, &mut scratch)
}

/// Complete a legacy unit fixture that was assembled below the checked-tree
/// construction boundary, then run the ordinary independent validator.
/// Production checked trees never use this helper.
#[cfg(test)]
pub(crate) fn check_unretained_borrow_fixture_facts(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut scratch = facts.clone();
    borrows::initialize_checked_direct_borrow_resources(
        program,
        &mut scratch,
        &crate::flow::StateMutationSummaryCache::default(),
    )?;
    borrows::initialize_checked_borrow_call_certificates(program, &mut scratch);
    check_checked_facts_recording(program, &mut scratch)
}

pub(crate) fn initialize_checked_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    borrows::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

pub(crate) fn initialize_checked_borrow_call_certificates(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
) {
    borrows::initialize_checked_borrow_call_certificates(program, facts)
}

#[cfg(test)]
pub(crate) fn check_checked_facts_recording(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    check_checked_facts_recording_with_crash_admission(
        program,
        facts,
        true,
        &crate::flow::StateMutationSummaryCache::default(),
    )
}

/// Check-stage entry for callers that already retain the pass's shared
/// mutation summary table. The summaries depend only on the immutable typed
/// program and the borrow facts, so the same table answers fact construction,
/// resource replay, borrow statements, and range indexing in one check pass.
pub(crate) fn check_checked_facts_recording_with_mutation_summaries(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    check_checked_facts_recording_with_crash_admission(program, facts, true, mutation_summaries)
}

#[cfg(test)]
pub(crate) fn check_checked_facts_recording_without_crash_admission(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    check_checked_facts_recording_with_crash_admission(
        program,
        facts,
        false,
        &crate::flow::StateMutationSummaryCache::default(),
    )
}

fn check_checked_facts_recording_with_crash_admission(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
    enforce_crash_admission: bool,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards =
        ranges::incoming_guards::IncomingGuardIndex::build(program, call_frames.as_ref());

    if let Err(mut evidence_diagnostics) = contracts::bind_call_evidence_arguments(program, facts) {
        diagnostics.append(&mut evidence_diagnostics);
    }

    if let Err(mut borrow_diagnostics) =
        borrows::check_flow_call_borrows(program, facts, mutation_summaries, call_frames.as_ref())
    {
        diagnostics.append(&mut borrow_diagnostics);
    }

    if let Err(mut contract_diagnostics) =
        contracts::check_flow_call_contracts(program, facts, &incoming_guards, call_frames.as_ref())
    {
        diagnostics.append(&mut contract_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts, &incoming_guards)
    {
        diagnostics.append(&mut multiplicity_diagnostics);
    }

    if let Err(mut crash_edge_diagnostics) = crashes::check_crash_exit_edge_isolation(program) {
        diagnostics.append(&mut crash_edge_diagnostics);
    }

    if let Err(mut operator_crash_diagnostics) =
        crashes::check_operator_invocation_custody(program, facts)
    {
        diagnostics.append(&mut operator_crash_diagnostics);
    }
    crashes::infer_path_conditioned_guard_coverage(program, facts, &incoming_guards);
    if enforce_crash_admission
        && let Err(mut crash_diagnostics) =
            crashes::check_published_ceiling_coverage(program, facts)
    {
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

    if let Err(mut range_diagnostics) = ranges::check_indexed_accesses(
        program,
        &facts.operators,
        &facts.borrow,
        &facts.flow,
        call_frames.as_ref(),
        &incoming_guards,
        mutation_summaries,
    ) {
        diagnostics.append(&mut range_diagnostics);
    }

    if let Err(mut termination_diagnostics) =
        termination::check_machine_termination_with_call_frames(program, call_frames.as_ref())
    {
        diagnostics.append(&mut termination_diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
