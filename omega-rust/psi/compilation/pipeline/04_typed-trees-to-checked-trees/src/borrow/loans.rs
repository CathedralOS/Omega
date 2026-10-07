//! Statement loans: the borrows a `let` or an assignment forms. Each loan
//! records its owner (the local, and the field path inside it that holds the
//! reference), the place it borrows, its access kind and its lineage.
//!
//! `statement_borrow_loans` is the entry; `state::append_state_borrow_facts`
//! calls it for each statement in order and records every returned
//! `StatementBorrowLoan` as a loan fact. Loans come from:
//!
//! - a `let` of reference type (`reference_local_borrow_loans`);
//! - a `let` of borrow-carrying data (`borrow_carrying_data_loans`), one loan
//!   for each reference its initializer supplies (`aggregate`);
//! - an assignment to an earlier local, or to a field of one, whose target is
//!   a reference or borrow-carrying data, handled as the equivalent `let`
//!   with the field path prefixed to each loan's owner path.
//!
//! For a reference `let`, `returned_carriers` is tried first, for a value
//! taken from a call result whose fields carry borrows, and a `match` lends
//! each arm's target. Otherwise the borrowed place comes from the borrow,
//! recast, place expression or view-returning call
//! (`helper_call_borrow_loan_place`, which resolves the call's returned-view
//! source through `view_link`). A place rooted at a local that already holds
//! a loan in this state is rebased onto that loan's place, once per such
//! loan. `retained_reference_lineage` gives a cast, or a call that declares
//! no direct view source, `UnretainedDerived` unless it is an explicit
//! reborrow; otherwise a place that was not rebased is `DirectRoot`, an
//! explicit reborrow through a single retained parent loan is `Reborrow`,
//! and the rest are `UnretainedDerived`.
//!
//! `owner_paths` matches owner field paths against place segments, and
//! `types` answers whether a type is a reference and with which access.
//! Other modules use `borrow_initializer_expressions` (persistent storage
//! check), `returned_value_loans` (a returned aggregate's leaves for the
//! formed-aggregate check), `helper_call_borrow_loan_place` and
//! `call_declares_direct_view_source` (resource lineage replay), and
//! `types::reference_borrow_access_kind` (borrow check details).

use crate::borrow::view_link::is_borrow_carrying_data;
use arena::Handle;
use checked_trees::expression::ExpressionHandle;
use checked_trees::name::Identifier;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use aggregate::borrowed_initializers;
use owner_paths::{owner_path_from_place_segments, owner_path_matches};
pub(crate) use owner_paths::{owner_segment_matches_place_segment, store_destination_places};

mod aggregate;
mod mutable_view_results;
mod owner_paths;
mod returned_carriers;
mod stored_calls;
pub(crate) mod types;

use types::{is_reference_type, reference_borrow_access_kind};

mod reference_borrows;
use aggregate_loans::*;
pub(in crate::borrow) use aggregate_loans::{
    borrowed_initializer_loans, helper_call_aggregate_borrow_loans,
};
pub(crate) use call_views::argument_borrow_loan_place;
pub(in crate::borrow) use call_views::{
    call_view_return_source_at, call_view_signature, helper_call_borrow_loan_place,
};
use reference_borrows::*;

mod aggregate_loans;
mod call_views;

#[derive(Clone)]
pub(super) struct StatementBorrowLoan {
    pub(super) formation_expression: ExpressionHandle,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: accesses::BorrowAccessPlace,
    pub(super) source_owner_symbol: SymbolHandle,
    pub(super) lineage: checked_trees::BorrowLoanLineage,
    pub(super) kind: checked_trees::BorrowAccessKind,
    /// The loan's retained root was captured from a call result, not from a
    /// borrow expression. It resolves receivers like any direct root, but it
    /// is not borrow ancestry: children formed through it stay derived rather
    /// than claiming `Reborrow` lineage.
    pub(super) call_result: bool,
}

struct RebasedBorrowPlace {
    place: accesses::BorrowAccessPlace,
    source_owner_symbol: SymbolHandle,
    parent_loan: Handle<checked_trees::BorrowLoanFact>,
    parent_lineage_is_retained: bool,
}

/// The initializer expressions that supply the references structurally carried
/// by `type_reference`. Persistent-storage checking uses this same traversal so
/// its static-source exemption cannot disagree with local aggregate loan
/// attribution about which nested fields actually borrow.
pub(crate) fn borrow_initializer_expressions(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    borrowed_initializers(program, type_reference, expression, &[], &[])
        .into_iter()
        .map(|initializer| initializer.expression)
        .collect()
}

/// One reference leaf a returned value carries: its position in the value,
/// the source expression that formed it, the place it borrows and its access.
pub(crate) struct ReturnedLoan {
    pub(crate) formation_expression: ExpressionHandle,
    pub(crate) owner_path: Vec<BorrowOwnerSegment>,
    pub(crate) place: accesses::BorrowAccessPlace,
    pub(crate) kind: checked_trees::BorrowAccessKind,
}

/// The loans a borrow-carrying value returned at `statement_index` carries,
/// resolved exactly as for a local initialized from the returned expression,
/// against the loans its state formed before the return. A returned value has
/// no local owner, so these loans are never recorded as facts; they exist to
/// judge the value's leaves against each other. A bare reference result is a
/// single leaf and yields nothing here.
pub(crate) fn returned_value_loans(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Vec<ReturnedLoan> {
    if is_reference_type(program, state.return_type)
        || !is_borrow_carrying_data(program, state.return_type)
    {
        return Vec::new();
    }
    // The fact builder resolved each statement against the loans recorded
    // before it; the state's span keeps that order. Lineage is not read here,
    // so a call-result root needs no separate marker.
    let span_start = borrow_state.loans.start();
    let prior_loans: Vec<StateLoanTracker> = borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .iter()
        .zip(0u32..)
        .take_while(|(loan, _)| loan.statement_index < statement_index)
        .map(|(loan, offset)| StateLoanTracker {
            handle: Handle::from_parts(span_start.arena_index() + offset, span_start.generation()),
            owner_symbol: loan.owner_symbol,
            owner_name: program.symbols.name(loan.owner_symbol).into(),
            kind: loan.kind.clone(),
            lineage: loan.lineage.clone(),
            call_result: false,
            owner_path: borrow.loan_owner_path(loan).to_vec(),
            place: accesses::BorrowAccessPlace {
                root_symbol: loan.root_symbol,
                segments: borrow.loan_segments(loan).to_vec(),
            },
        })
        .collect();
    let returned_value = checked_trees::statement::TableLocalData {
        symbol: state.symbol,
        name: state.name.clone(),
        type_reference: state.return_type,
        initial_value: expression,
        is_mutable: false,
        type_is_inferred: false,
        relevance: language_core::BindingRelevance::Relevant,
    };
    borrow_carrying_data_loans(
        program,
        state,
        statement_index,
        machine_symbol,
        &returned_value,
        &prior_loans,
    )
    .into_iter()
    .map(|loan| ReturnedLoan {
        formation_expression: loan.formation_expression,
        owner_path: loan.owner_path,
        place: loan.place,
        kind: loan.kind,
    })
    .collect()
}

pub(super) fn statement_borrow_loans(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<StatementBorrowLoan> {
    match statement {
        StatementNode::Assignment(assignment) => assignment_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            assignment,
            loan_trackers,
        ),
        StatementNode::LocalData(local_data) => {
            // Borrow-carrying data local (`let msg: Message = Message { body: input }`):
            // the value borrows whatever its reference field is initialized from,
            // so its loan follows that source's place (decision 15 stage 2).
            if !is_reference_type(program, local_data.type_reference)
                && is_borrow_carrying_data(program, local_data.type_reference)
            {
                return borrow_carrying_data_loans(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    local_data,
                    loan_trackers,
                );
            }
            if !is_reference_type(program, local_data.type_reference)
                && !element_view_local_binding(program, state, machine_symbol, local_data)
            {
                return Vec::new();
            }

            reference_local_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                local_data,
                loan_trackers,
                true,
            )
        }
        StatementNode::Call(_) => {
            stored_calls::loans(program, state, machine_symbol, statement, call_frames)
        }
        StatementNode::RootBinding(_)
        | StatementNode::AssemblyFact(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => Vec::new(),
    }
}

/// A store installs the replacement's loans in its actual carrier. A projected
/// store through a local reference belongs to that reference's backing storage,
/// so ending the temporary reference cannot end loans installed in the parent.
/// These rows retain conflicts within the source state; they do not establish
/// outlives authority for transporting local storage into a successor.
fn assignment_borrow_loans(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(target) = borrow_access_place(
        program,
        state.symbol,
        statement_index,
        assignment.target,
        machine_symbol,
    ) else {
        return Vec::new();
    };
    let local = program
        .state_local_declaration(state, target.root_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)
        .map(|(_, local)| local);
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == target.root_symbol);
    let Some(owner_name) = local
        .map(|local| &local.name)
        .or_else(|| parameter.map(|parameter| &parameter.name))
    else {
        return Vec::new();
    };
    let Some(target_type) = crate::flow::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        assignment.target,
    ) else {
        return Vec::new();
    };
    if !is_reference_type(program, target_type) && !is_borrow_carrying_data(program, target_type) {
        return Vec::new();
    }
    let synthetic_local = checked_trees::statement::TableLocalData {
        symbol: target.root_symbol,
        name: owner_name.clone(),
        type_reference: target_type,
        initial_value: assignment.value,
        is_mutable: true,
        type_is_inferred: false,
        relevance: language_core::BindingRelevance::Relevant,
    };

    let loans = if is_reference_type(program, target_type) {
        reference_local_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
            false,
        )
    } else {
        borrow_carrying_data_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
        )
    };
    let destinations = store_destination_places(
        program,
        state,
        target,
        loan_trackers.iter().map(|loan| {
            (
                loan.owner_symbol,
                loan.owner_path.as_slice(),
                loan.place.root_symbol,
                loan.place.segments.as_slice(),
            )
        }),
    );
    destinations
        .iter()
        .flat_map(|destination| {
            let owner_path = owner_path_from_place_segments(program, &destination.segments);
            loans.iter().cloned().map(move |mut loan| {
                loan.owner_symbol = destination.root_symbol;
                loan.owner_name = program.symbols.name(destination.root_symbol).into();
                let mut path = owner_path.clone();
                path.append(&mut loan.owner_path);
                loan.owner_path = path;
                loan
            })
        })
        .collect()
}
