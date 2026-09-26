//! The checks over checked facts. Each check reads the typed program and the
//! fact tables that `facts` built, and reports a diagnostic where a call,
//! borrow, ownership transfer, crash route, operator use, indexed access or
//! ranked cycle fails its obligation.
//!
//! `checking::lower_typed_trees` enters here three times: it calls
//! `initialize_checked_direct_borrow_resources` and
//! `initialize_checked_borrow_call_certificates` to fill the borrow evidence
//! ledgers, then `check_checked_facts_recording_with_mutation_summaries` to
//! run the checks. `replay_checked_borrow_certificates`, re-exported from the
//! crate root, runs only the borrow check, on a copy of published facts.
//!
//! `check_checked_facts_recording_with_crash_admission` runs every check and
//! collects all diagnostics instead of stopping at the first failure. It
//! first builds the call frame resolver and the incoming guard index
//! (`ranges::incoming_guards`) that several checks share, then runs:
//!
//! 1. `contracts::bind_call_evidence_arguments` binds the evidence arguments
//!    of contract expressions and calls;
//! 2. `borrows` checks call and statement borrows and replays the retained
//!    borrow certificates;
//! 3. `contracts::check_flow_call_contracts` checks call requirements, exit
//!    guarantees, arrival requirements and domain field writes;
//! 4. `multiplicity` checks linear obligations;
//! 5. `crashes` checks crash exit edge isolation and operator invocation
//!    custody, infers path-conditioned guard coverage, and, when crash
//!    admission is enforced, checks published ceiling coverage;
//! 6. `content` infers identity-preserving reshuffles and composes partition
//!    wrappers, then checks retained content custody;
//! 7. `carry` rejects a call that may suspend while a suspension-forbidden
//!    value is live;
//! 8. `operators` checks operator resolution;
//! 9. `ranges` checks indexed accesses;
//! 10. `termination` checks each terminating machine's ranking.
//!
//! `contracts` and `termination` are also used outside this module (by proof,
//! fact construction, package review and execution planning, among others),
//! and `multiplicity` and `operators` export queries used elsewhere in the
//! crate.

// Call obligations: contract requirements and guarantees, and borrows.
mod borrows;
pub(crate) mod contracts;

// Ownership and custody: linear obligations, retained content, and values
// live across a suspending call.
mod carry;
mod content;
mod multiplicity;

// Crash routes.
mod crashes;

// Operator resolution, indexed access bounds, and machine termination.
mod operators;
mod ranges;
pub(crate) mod termination;

use diagnostics::Diagnostic;
pub(crate) use operators::{named_operator_route_is_false, operator_route_is_false};
use symbols::SymbolHandle;

pub(crate) use multiplicity::{
    nominal_drop_machine_symbol, type_carries_linear_obligation, type_multiplicity,
};
pub(crate) use ranges::enter_root_currency_scope;
pub(crate) use ranges::incoming_guards::IncomingGuardIndexCache;

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
    borrows::initialize_checked_borrow_call_certificates(
        program,
        &mut scratch,
        &IncomingGuardIndexCache::default(),
    );
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
    guard_index: &IncomingGuardIndexCache,
) {
    borrows::initialize_checked_borrow_call_certificates(program, facts, guard_index)
}

/// Independently replay the borrow evidence retained in one published
/// checked fact table. The certificate ledgers and loan resource rows cross
/// the publication boundary inside the checked trees as records, not as
/// trusted authority: this entry rebuilds the pass's auxiliary indexes from
/// the typed program, re-derives every formation's subjects and consulted
/// premise scope, and consumes each retained row exactly once. The replay
/// runs on a scratch copy of the published facts, so the consumer's record
/// stays read-only; a drifted, duplicated, retargeted or unconsumed row is
/// an ordinary diagnostic, never a rebuild in place.
pub fn replay_checked_borrow_certificates(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut scratch = facts.clone();
    let call_frames = validation::CallFrameResolver::new(program);
    let guard_index = IncomingGuardIndexCache::default();
    let incoming_guards = guard_index.index(program, call_frames.as_ref());
    borrows::check_flow_call_borrows(
        program,
        &mut scratch,
        &crate::flow::StateMutationSummaryCache::default(),
        call_frames.as_ref(),
        incoming_guards,
    )
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
        &IncomingGuardIndexCache::default(),
        &[],
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
    guard_index: &IncomingGuardIndexCache,
    proven_machine_contracts: &[(SymbolHandle, Vec<typed_trees::expression::ExpressionHandle>)],
) -> Result<(), Vec<Diagnostic>> {
    check_checked_facts_recording_with_crash_admission(
        program,
        facts,
        true,
        mutation_summaries,
        guard_index,
        proven_machine_contracts,
    )
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
        &IncomingGuardIndexCache::default(),
        &[],
    )
}

fn check_checked_facts_recording_with_crash_admission(
    program: &typed_trees::TypedTrees,
    facts: &mut checked_trees::CheckFacts,
    enforce_crash_admission: bool,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    guard_index: &IncomingGuardIndexCache,
    proven_machine_contracts: &[(SymbolHandle, Vec<typed_trees::expression::ExpressionHandle>)],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = guard_index.index(program, call_frames.as_ref());

    if let Err(mut evidence_diagnostics) = contracts::bind_call_evidence_arguments(program, facts) {
        diagnostics.append(&mut evidence_diagnostics);
    }

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(
        program,
        facts,
        mutation_summaries,
        call_frames.as_ref(),
        incoming_guards,
    ) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    if let Err(mut contract_diagnostics) = contracts::check_flow_call_contracts(
        program,
        facts,
        incoming_guards,
        call_frames.as_ref(),
        proven_machine_contracts,
    ) {
        diagnostics.append(&mut contract_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts, incoming_guards)
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
    crashes::infer_path_conditioned_guard_coverage(program, facts, incoming_guards);
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
        incoming_guards,
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
