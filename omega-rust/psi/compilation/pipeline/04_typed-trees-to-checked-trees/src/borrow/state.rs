use checked_trees::BorrowLoanFact;
use checked_trees::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowWritableRootFact, StateBorrowFact,
};

use super::calls::collect_statement_borrow_calls;
use super::last_uses::update_state_loan_last_uses;
use super::loans::{StatementBorrowLoan, statement_borrow_loans};
use super::roots::{append_state_writable_roots, mutable_parameter_count};
use super::tracker::StateLoanTracker;

pub(super) struct BorrowFactArenas<'arenas> {
    pub(super) incoming_references:
        &'arenas mut arena::Arena<checked_trees::IncomingReferenceIdentity>,
    pub(super) writable_roots: &'arenas mut arena::Arena<BorrowWritableRootFact>,
    pub(super) access_segments: &'arenas mut arena::Arena<facts::PlaceSegment>,
    pub(super) owner_segments: &'arenas mut arena::Arena<checked_trees::BorrowLoanOwnerSegment>,
    pub(super) argument_accesses: &'arenas mut arena::Arena<BorrowArgumentAccessFact>,
    pub(super) calls: &'arenas mut arena::Arena<BorrowCallFact>,
    pub(super) loans: &'arenas mut arena::Arena<BorrowLoanFact>,
    pub(super) states: &'arenas mut arena::Arena<StateBorrowFact>,
}

pub(super) fn append_state_borrow_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    arenas: &mut BorrowFactArenas<'_>,
    state_loan_trackers: &mut Vec<StateLoanTracker>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    state_loan_trackers.clear();
    append_incoming_reference_trackers(program, machine, state, arenas, state_loan_trackers);
    // Formation reads the carrier's current contents, while last-use analysis
    // retains every earlier occurrence. Replacement retires the old slot only
    // after its RHS has been resolved against the entry roster.
    let mut current_loans = state_loan_trackers.clone();
    let mut writable_roots_span = arena::HandleSpan::empty();
    append_state_writable_roots(
        program,
        machine,
        state,
        arenas.writable_roots,
        &mut writable_roots_span,
    );

    let mut calls_span = arena::HandleSpan::empty();
    let mut loans_span = arena::HandleSpan::empty();
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let pending_loans = statement_borrow_loans(
            program,
            state,
            statement_index,
            machine.symbol,
            statement,
            &current_loans,
            call_frames,
        );
        retire_replaced_carrier_loans(
            program,
            machine,
            state,
            statement_index,
            statement,
            call_frames,
            &mut current_loans,
        );
        for pending in grouped_carrier_loans(pending_loans) {
            let loan_segments = arenas
                .access_segments
                .insert_many(pending.place.segments.clone());
            let owner_path = arenas
                .owner_segments
                .insert_many(pending.owner_path.iter().copied());
            let handle = arenas.loans.append_to_span(
                &mut loans_span,
                BorrowLoanFact {
                    incoming_reference: pending.place.incoming_reference,
                    statement_index,
                    formation_expression: pending.formation_expression,
                    last_use_statement_index: statement_index,
                    owner_symbol: pending.owner_symbol,
                    owner_path,
                    source_owner_symbol: pending.source_owner_symbol,
                    lineage: pending.lineage.clone(),
                    root_symbol: pending.place.root_symbol,
                    segments: loan_segments,
                    kind: pending.kind.clone(),
                },
            );
            let tracker = StateLoanTracker {
                handle,
                owner_symbol: pending.owner_symbol,
                owner_name: pending.owner_name,
                kind: pending.kind,
                lineage: pending.lineage,
                call_result: pending.call_result,
                owner_path: pending.owner_path,
                place: pending.place,
            };
            current_loans.push(tracker.clone());
            state_loan_trackers.push(tracker);
        }
        let mut call_ordinal = 0usize;
        collect_statement_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            statement,
            &mut call_ordinal,
            arenas.access_segments,
            arenas.argument_accesses,
            arenas.calls,
            &mut calls_span,
        );
    }

    update_state_loan_last_uses(
        program,
        state.symbol,
        state.statement_nodes,
        arenas.calls.span_or_empty(calls_span),
        arenas.access_segments,
        arenas.argument_accesses,
        state_loan_trackers,
        arenas.loans,
    );

    arenas.states.append(StateBorrowFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: writable_roots_span,
        mutable_parameter_count: mutable_parameter_count(program, state),
        calls: calls_span,
        loans: loans_span,
    });
}

/// Arena layout groups one descriptor's alternative origins contiguously.
/// This is allocation order, not execution order: formation expressions and
/// statement boundaries still name each event. Repeated declared candidates
/// form a set, so forwarding a group cannot multiply identical obligations.
fn grouped_carrier_loans(mut loans: Vec<StatementBorrowLoan>) -> Vec<StatementBorrowLoan> {
    loans.sort_by_cached_key(|loan| {
        (
            loan.owner_symbol.arena_index(),
            loan.owner_symbol.generation(),
            loan.owner_path
                .iter()
                .map(|segment| match segment {
                    checked_trees::BorrowLoanOwnerSegment::Field(symbol) => {
                        (0u8, symbol.arena_index() as u64, symbol.generation())
                    }
                    checked_trees::BorrowLoanOwnerSegment::Case(symbol) => {
                        (1u8, symbol.arena_index() as u64, symbol.generation())
                    }
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(element) => {
                        (2u8, *element as u64, 0)
                    }
                    checked_trees::BorrowLoanOwnerSegment::DynamicIndex => (3u8, 0, 0),
                })
                .collect::<Vec<_>>(),
        )
    });
    let mut grouped: Vec<StatementBorrowLoan> = Vec::with_capacity(loans.len());
    for loan in loans {
        let duplicate = grouped
            .iter()
            .rev()
            .take_while(|previous| {
                previous.owner_symbol == loan.owner_symbol && previous.owner_path == loan.owner_path
            })
            .any(|previous| {
                previous.formation_expression == loan.formation_expression
                    && previous.source_owner_symbol == loan.source_owner_symbol
                    && previous.place == loan.place
                    && previous.kind == loan.kind
                    && previous.lineage == loan.lineage
                    && previous.call_result == loan.call_result
            });
        if !duplicate {
            grouped.push(loan);
        }
    }
    grouped
}

/// A write through a reference retains its descriptor. Replacing a carrier
/// removes only the old leaves within that exact destination; earlier facts
/// remain available to the flow lifetime ledger and already detached values.
fn retire_replaced_carrier_loans(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    current: &mut Vec<StateLoanTracker>,
) {
    let Some(assignment) = statement.as_assignment() else {
        return;
    };
    if validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
        .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
        && call_frames
            .and_then(|frames| frames.assignment_replaces_reference_binding(machine, statement))
            != Some(true)
    {
        return;
    }
    let Some(target) = super::accesses::borrow_access_place(
        program,
        state.symbol,
        statement_index,
        assignment.target,
        machine.symbol,
    ) else {
        return;
    };
    let destinations = super::loans::store_destination_places(
        program,
        state,
        target,
        current.iter().map(|loan| {
            (
                loan.owner_symbol,
                loan.owner_path.as_slice(),
                loan.place.incoming_reference,
                loan.place.root_symbol,
                loan.place.segments.as_slice(),
            )
        }),
    );
    let [destination] = destinations.as_slice() else {
        // A may-alias target cannot identify the replaced descriptor slot.
        return;
    };
    current.retain(|loan| {
        loan.owner_symbol != destination.root_symbol
            || !super::loans::place_path_matches_owner_prefix(
                program,
                &destination.segments,
                &loan.owner_path,
            )
    });
}

/// Entry permissions precede statement loans: carrying a descriptor into this
/// invocation did not borrow its containing slot. Only a later establishment
/// gets a loan row and activation; transfers retain the entry's opaque identity.
fn append_incoming_reference_trackers(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    arenas: &mut BorrowFactArenas<'_>,
    trackers: &mut Vec<StateLoanTracker>,
) {
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const && !parameter.relevance.is_erased())
    {
        // The root reference's permission is the existing borrowed parameter
        // contract. Its referent may also contain independently carried leaves.
        let Some(carrier) = validation::reference_result_custody::input_carrier_type(
            program,
            parameter.type_reference,
        ) else {
            continue;
        };
        let Some(sources) = validation::reference_result_custody::record_carrier_sources(
            program,
            carrier,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: parameter.symbol,
            },
        ) else {
            continue;
        };
        for source in sources {
            let Some(owner_path) = validation::reference_result_custody::loan_owner_path(
                program,
                carrier,
                &source.path,
            ) else {
                continue;
            };
            let Some(kind) = source.source.access.borrow_access_kind() else {
                continue;
            };
            let path = arenas
                .owner_segments
                .insert_many(owner_path.iter().copied());
            let incoming_reference =
                arenas
                    .incoming_references
                    .append(checked_trees::IncomingReferenceIdentity {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        parameter_symbol: parameter.symbol,
                        carrier_type: carrier,
                        owner_path: path,
                    });
            trackers.push(StateLoanTracker {
                handle: arena::Handle::invalid(),
                owner_symbol: parameter.symbol,
                owner_name: parameter.name.clone(),
                kind,
                lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                call_result: false,
                owner_path,
                place: checked_trees::CapturedPlace {
                    incoming_reference,
                    root_symbol: parameter.symbol,
                    segments: Vec::new(),
                },
            });
        }
    }
}
