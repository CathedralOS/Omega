//! Borrow facts: the writable roots, loans, borrowing calls and argument
//! accesses of every state, recorded as `checked_trees::BorrowFacts` for the
//! later fact builders and the borrow checks in `checks::borrows`.
//!
//! `build_borrow_facts` is the entry; `facts::build_check_facts` calls it
//! first. It sizes the arenas (`roots`), then for every state of every
//! machine `state::append_state_borrow_facts` records, in order: the state's
//! writable roots (`roots`); for each statement, the loans it forms
//! (`loans`), each tracked as a `tracker::StateLoanTracker`, and its
//! borrowing calls (`calls`, which records their argument accesses through
//! `accesses`); then each loan's last use (`last_uses`); and finally the
//! state's `StateBorrowFact` row.
//!
//! `view_link` resolves which input a returned view borrows; `loans` uses it
//! to link a call result's loan to its argument, and `checks::borrows` uses
//! it for the returned-view checks. `last_uses` and `loans` also export
//! queries used elsewhere in the crate.
//!
//! Stored input descriptors seed opaque referent identities, not loans of the
//! parameter's descriptor slots. Extraction and ordinary transfers preserve
//! those identities in actual statement loans. Formation reads current carrier
//! contents; replacement retires that slot's old occurrence after its RHS,
//! while the complete historical ledger still owns lifetime analysis.
//! Normal result leaves are recorded separately at formation, using that same
//! current roster. Escape and simultaneous-hold checks consume those rows;
//! they never rebuild a return's contents from historical local activations.

use validation::machine_state_count;

use checked_trees::BorrowFacts;
pub(crate) mod accesses;
pub(crate) mod calls;
mod children;
mod last_uses;
mod loans;
mod owner_geometry;
mod roots;
mod state;
mod tracker;
pub(crate) use validation::borrow_contracts as view_link;

pub(crate) use last_uses::{owner_path_overlaps_place_segments, place_is_used_after_statement};
pub(crate) use loans::argument_borrow_loan_place;
pub(crate) use loans::borrow_initializer_expressions;
pub(crate) use loans::call_reference_sources;
pub(crate) use loans::owner_path_from_place_segments;
pub(crate) use loans::owner_segment_matches_place_segment;
pub(crate) use loans::place_path_covers_owner;
pub(crate) use loans::returned_body_source_mismatch;
pub(crate) use loans::store_destination_places;
pub(crate) use loans::types::reference_borrow_access_kind;
pub(crate) use owner_geometry::carrier_path;
pub(crate) use tracker::BorrowOwnerSegment;

use roots::estimated_borrow_root_capacity;
use state::{BorrowFactArenas, append_state_borrow_facts};

/// The caller prepares call frames once for this immutable program. `None`
/// retains failed preparation; source reconstruction must not retry it per call.
pub(crate) fn build_borrow_facts(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> BorrowFacts {
    let mut writable_roots = arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut incoming_references = arena::Arena::new();
    let mut access_segments =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut owner_segments =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls = arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans = arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut returned_loans = arena::Arena::new();
    let mut states = arena::Arena::with_capacity(machine_state_count(program));
    let mut state_loan_trackers = Vec::new();

    {
        let mut arenas = BorrowFactArenas {
            incoming_references: &mut incoming_references,
            writable_roots: &mut writable_roots,
            access_segments: &mut access_segments,
            owner_segments: &mut owner_segments,
            argument_accesses: &mut argument_accesses,
            calls: &mut calls,
            loans: &mut loans,
            returned_loans: &mut returned_loans,
            states: &mut states,
        };
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                append_state_borrow_facts(
                    program,
                    machine,
                    state,
                    &mut arenas,
                    &mut state_loan_trackers,
                    call_frames,
                );
            }
        }
    }

    BorrowFacts::with_roots(
        incoming_references,
        writable_roots,
        access_segments,
        owner_segments,
        argument_accesses,
        calls,
        loans,
        returned_loans,
        states,
    )
}
