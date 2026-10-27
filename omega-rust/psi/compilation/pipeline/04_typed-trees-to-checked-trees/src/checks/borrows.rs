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
mod captured_descriptors;
mod completed_custody;
pub(crate) use completed_custody::require_unborrowed_completion;
pub(crate) mod calls;
mod moves;
mod retained_outputs;
pub(crate) use retained_outputs::retained_outputs_are_attributed;
mod reads;
mod resources;
mod selected_retirement;
mod staged_values;
mod statements;
mod successor_cleanup;

// Shared by the checks: compatibility judgments, and binding and loan
// details.
mod access_sources;
mod aliases;
mod completed_backings;
mod shared_observations;
pub(crate) use shared_observations::shared_reference_observation;
mod details;
mod overlap;
mod place_access;
pub(crate) use overlap::{
    StorageSelectorFacts, captured_storage_index, captured_storage_paths_are_disjoint,
    storage_path_compatibility,
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
    let mut borrow_by_state = symbols::SymbolKeyMap::<
        (symbols::SymbolHandle, symbols::SymbolHandle),
        &checked_trees::StateBorrowFact,
    >::default();
    for (_, state_borrow) in facts.borrow.states.iter() {
        borrow_by_state
            .entry((state_borrow.machine_symbol, state_borrow.state_symbol))
            .or_insert(state_borrow);
    }
    let state_flows: Vec<(&FlowStateFact, Option<&checked_trees::StateBorrowFact>)> = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state_flow)| {
            let borrow_state = borrow_by_state
                .get(&(state_flow.machine_symbol, state_flow.state_symbol))
                .copied();
            (state_flow, borrow_state)
        })
        .collect();

    calls::check_compatibility(
        program,
        facts,
        &state_flows,
        incoming_guards,
        call_frames,
        &mut diagnostics,
    );

    check_view_return_elision(program, facts, &mut diagnostics);
    check_view_return_escape(program, facts, &mut diagnostics);
    check_persistent_borrow_assignments(program, facts, call_frames, &mut diagnostics);

    // Receiver read footprints serve only this pass's whole-receiver call
    // judgments; they build on the first call that needs them.
    let receiver_reads = crate::flow::StateReceiverReadCache::default();
    let per_state_diagnostics: Vec<Vec<Diagnostic>> = validation::checking_thread_pool().install(|| {
        use rayon::prelude::*;
        state_flows
            .par_iter()
            .filter_map(|&(state_flow, borrow_state)| {
                let borrow_state = borrow_state?;
                let mut local_diagnostics = Vec::new();
                let mut local_bound_lookup = None;

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
                        &mut local_bound_lookup,
                    )
                })
                .unwrap_or_default();

                check_statement_borrows(
                    program,
                    facts,
                    state_flow,
                    borrow_state,
                    &stated_premises,
                    &mut local_diagnostics,
                    mutation_summaries,
                    &receiver_reads,
                    call_frames,
                    &mut local_bound_lookup,
                );
                aggregates::check_formed_aggregates(
                    program,
                    facts,
                    state_flow,
                    borrow_state,
                    &stated_premises,
                    call_frames,
                    &mut local_diagnostics,
                    &mut local_bound_lookup,
                );
                (!local_diagnostics.is_empty()).then_some(local_diagnostics)
            })
            .collect()
    });
    for local_diagnostics in per_state_diagnostics {
        diagnostics.extend(local_diagnostics);
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
    facts
        .borrow
        .first_state_borrow(state_flow.machine_symbol, state_flow.state_symbol)
        .map(|(_, state)| state)
}
