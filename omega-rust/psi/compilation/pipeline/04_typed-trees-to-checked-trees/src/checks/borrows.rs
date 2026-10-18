//! Borrow checks over the facts constructed once by the check stage.
//! Calls and statements judge conflicts at their actual evaluation points;
//! reads, writes, call operands and exclusive operations share one live-loan
//! judgment over the storage they may denote (`place_access`); a call's
//! operands and an aggregate's reference leaves share one simultaneous-hold
//! judgment; returned views and stored carriers are checked for escape and
//! persistence.
//! Resource rows are prepared before this pass for lowering. Compatibility
//! results are transient judgments, not certificates to serialize and replay.
//! Compatibility also consumes completed scalar observations at each use;
//! immutable owned values retain identity, while mutable storage and borrowed
//! referents require their existing version and lineage evidence.

// Checks over signatures and bodies that retain no evidence.
mod elision;
mod escape;
mod persistent;

// Call, aggregate formation and statement judgments, plus resource rows
// consumed by lowering.
mod aggregates;
pub(crate) mod calls;
mod moves;
mod reads;
mod resources;
mod statements;

// Shared by the checks: compatibility judgments, and binding and loan
// details.
mod access_sources;
mod aliases;
mod details;
mod overlap;
mod place_access;
pub(crate) use overlap::{
    StorageSelectorFacts, captured_storage_paths_are_disjoint, storage_path_compatibility,
};
mod simultaneous;

use super::ranges::incoming_guards::IncomingGuardIndex;
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use self::elision::check_view_return_elision;
use self::escape::check_view_return_escape;
use self::persistent::check_persistent_borrow_assignments;
use self::statements::check_statement_borrows;

pub(crate) fn check_flow_call_borrows(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    // The whole-program bound index builds on first use and is shared across
    // every premise collection this pass performs.
    let mut bound_lookup = None;
    calls::check_compatibility(
        program,
        facts,
        incoming_guards,
        call_frames,
        &mut diagnostics,
        &mut bound_lookup,
    );

    check_view_return_elision(program, facts, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, facts, call_frames, &mut diagnostics);

    // Receiver read footprints serve only this pass's whole-receiver call
    // judgments; they build on the first call that needs them.
    let receiver_reads = crate::flow::StateReceiverReadCache::default();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(borrow_state) = matching_borrow_state(facts, state_flow) else {
            continue;
        };

        // Entry premises are stable inside the state. Statement-local call
        // guarantees are appended by the consumer at the exact use point.
        let stated_premises = validation::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        )
        .and_then(|state| {
            program
                .machine_by_symbol(state_flow.machine_symbol)
                .map(|machine| (machine, state))
        })
        .map(|(machine, state)| {
            overlap::stated_ordering_premises(
                program,
                facts,
                machine,
                state,
                incoming_guards,
                &mut bound_lookup,
            )
        })
        .unwrap_or_default();

        check_statement_borrows(
            program,
            facts,
            state_flow,
            borrow_state,
            &stated_premises,
            &mut diagnostics,
            mutation_summaries,
            &receiver_reads,
            call_frames,
            &mut bound_lookup,
        );
        aggregates::check_formed_aggregates(
            program,
            facts,
            state_flow,
            borrow_state,
            &stated_premises,
            call_frames,
            &mut diagnostics,
            &mut bound_lookup,
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    resources::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

fn matching_borrow_state<'a>(
    facts: &'a CheckFacts,
    state_flow: &FlowStateFact,
) -> Option<&'a checked_trees::StateBorrowFact> {
    crate::flow::borrow_state_fact(
        &facts.borrow,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )
    .map(|(_, state)| state)
}
