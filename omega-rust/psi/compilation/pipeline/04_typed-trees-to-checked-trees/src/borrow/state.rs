use checked_trees::BorrowLoanFact;
use checked_trees::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowWritableRootFact, StateBorrowFact,
};

use super::calls::collect_statement_borrow_calls;
use super::last_uses::update_state_loan_last_uses;
use super::loans::{StatementBorrowLoan, returned_value_loans, statement_borrow_loans};
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
    pub(super) loan_suppliers: &'arenas mut arena::Arena<arena::Handle<BorrowLoanFact>>,
    pub(super) returned_loans: &'arenas mut arena::Arena<checked_trees::BorrowReturnedLoanFact>,
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
    let mut returned_loans_span = arena::HandleSpan::empty();
    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        // Establish computed storage before any descriptor consumes its
        // contents, including the state's normal result. Child homes belong
        // to this same current roster, not a second return-source analysis.
        let mut backing_loans = Vec::new();
        super::loans::retain_backing_contents(
            program,
            state,
            statement_index,
            machine.symbol,
            statement,
            &mut backing_loans,
            &current_loans,
        );
        append_carrier_loans(
            arenas,
            &mut loans_span,
            &mut current_loans,
            state_loan_trackers,
            backing_loans,
            statement_index,
        );
        // Capture result leaves against the same current roster as a local or
        // store. Reconstructing them later from historical loans loses entry
        // referents and revives overwritten carrier contents.
        for (transition_target, returned) in returned_value_loans(
            program,
            state,
            statement_index,
            machine.symbol,
            statement,
            &current_loans,
        ) {
            let segments = arenas.access_segments.insert_many(returned.place.segments);
            let owner_path = arenas.owner_segments.insert_many(returned.owner_path);
            arenas.returned_loans.append_to_span(
                &mut returned_loans_span,
                checked_trees::BorrowReturnedLoanFact {
                    statement_index,
                    transition_target,
                    formation_expression: returned.formation_expression,
                    incoming_reference: returned.place.incoming_reference,
                    root: returned.place.root,
                    segments,
                    owner_path,
                    kind: returned.kind,
                },
            );
        }
        let replacement = super::loans::capture_replacement(
            program,
            machine,
            state,
            statement_index,
            statement,
            call_frames,
            &current_loans,
        );
        let pending_loans = statement_borrow_loans(
            program,
            state,
            statement_index,
            machine.symbol,
            statement,
            &current_loans,
            call_frames,
            replacement.as_ref(),
        );
        retire_replaced_carrier_loans(
            program,
            machine,
            statement,
            call_frames,
            replacement.as_ref(),
            &mut current_loans,
        );
        append_carrier_loans(
            arenas,
            &mut loans_span,
            &mut current_loans,
            state_loan_trackers,
            pending_loans,
            statement_index,
        );
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
        returned_loans: returned_loans_span,
    });
}

fn append_carrier_loans(
    arenas: &mut BorrowFactArenas<'_>,
    loans_span: &mut arena::HandleSpan<BorrowLoanFact>,
    current: &mut Vec<StateLoanTracker>,
    history: &mut Vec<StateLoanTracker>,
    pending: Vec<StatementBorrowLoan>,
    statement_index: usize,
) {
    for pending in grouped_carrier_loans(pending) {
        let segments = arenas
            .access_segments
            .insert_many(pending.place.segments.clone());
        let owner_path = arenas
            .owner_segments
            .insert_many(pending.owner_path.iter().copied());
        let handle = arenas.loans.append_to_span(
            loans_span,
            BorrowLoanFact {
                incoming_reference: pending.place.incoming_reference,
                statement_index,
                formation_expression: pending.formation_expression,
                retained_operand: pending.retained_operand,
                last_use_statement_index: statement_index,
                owner_symbol: pending.owner_symbol,
                storage_owner: pending.storage_owner,
                owner_path,
                source_owner_symbol: pending.source_owner_symbol,
                supplying_loans: arenas.loan_suppliers.insert_many(pending.supplying_loans),
                lineage: pending.lineage.clone(),
                root: pending.place.root,
                segments,
                kind: pending.kind.clone(),
            },
        );
        let tracker = StateLoanTracker {
            handle,
            owner_symbol: pending.owner_symbol,
            storage_owner: pending.storage_owner,
            owner_name: pending.owner_name,
            kind: pending.kind,
            lineage: pending.lineage,
            call_result: pending.call_result,
            owner_path: pending.owner_path,
            place: pending.place,
            retained_operand: pending.retained_operand,
        };
        current.push(tracker.clone());
        history.push(tracker);
    }
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
            match loan.storage_owner {
                facts::PlaceRoot::Symbol(symbol) => {
                    (0u8, symbol.arena_index(), symbol.generation())
                }
                facts::PlaceRoot::Expression(expression) => {
                    (1u8, expression.arena_index(), expression.generation())
                }
                facts::PlaceRoot::TypeReference(reference) => {
                    (2u8, reference.arena_index(), reference.generation())
                }
                facts::PlaceRoot::Unknown => (3u8, 0, 0),
            },
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
            .iter_mut()
            .rev()
            .take_while(|previous| {
                previous.owner_symbol == loan.owner_symbol && previous.owner_path == loan.owner_path
            })
            .find(|previous| {
                previous.formation_expression == loan.formation_expression
                    && previous.storage_owner == loan.storage_owner
                    && previous.source_owner_symbol == loan.source_owner_symbol
                    && previous.supplying_loans.is_empty() == loan.supplying_loans.is_empty()
                    && (previous.supplying_loans == loan.supplying_loans
                        || !loan
                            .owner_path
                            .contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex))
                    && previous.place == loan.place
                    && previous.retained_operand == loan.retained_operand
                    && previous.kind == loan.kind
                    && previous.lineage == loan.lineage
                    && previous.call_result == loan.call_result
            });
        if let Some(previous) = duplicate {
            // Equal current alternatives share their immediate predecessor
            // edges. Keep a DAG rather than copying every route's history.
            for supplier in loan.supplying_loans {
                if !previous.supplying_loans.contains(&supplier) {
                    previous.supplying_loans.push(supplier);
                }
            }
        } else {
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
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    replacement: Option<&super::loans::CarrierReplacement>,
    current: &mut Vec<StateLoanTracker>,
) {
    if let Some(effects) =
        call_frames.and_then(|frames| frames.stored_reference_effects(machine, statement))
    {
        // Exact effects install a complete post-call descriptor, including
        // preservation alternatives. Retire the former occurrence even when
        // one route kept its value; keeping both would turn alternatives into
        // competing loans. Weak stores cannot retire unselected physical slots.
        current.retain(|loan| {
            !effects.iter().any(|effect| {
                !effect.may_select
                    && loan.storage_owner
                        == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                    && super::loans::place_path_covers_owner(
                        program,
                        &effect.destination.local_segments,
                        &loan.owner_path,
                    )
            })
        });
    }
    if let Some(replacement) = replacement {
        current.retain(|loan| !replacement.replaces(program, loan.storage_owner, &loan.owner_path));
    }
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
        let Some(sources) =
            validation::reference_result_custody::declared_carrier_references(program, carrier)
        else {
            continue;
        };
        for source in sources {
            let Some(segments) = validation::reference_result_custody::origin_place_segments(
                program,
                carrier,
                source.path.iter().cloned(),
            ) else {
                continue;
            };
            let owner_path = super::owner_path_from_place_segments(program, &segments);
            let Some(kind) = source.access.borrow_access_kind() else {
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
                storage_owner: facts::PlaceRoot::Symbol(parameter.symbol),
                owner_name: parameter.name.clone(),
                kind,
                lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                call_result: false,
                owner_path,
                retained_operand: None,
                place: checked_trees::CapturedPlace {
                    incoming_reference,
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments: Vec::new(),
                },
            });
        }
    }
}
