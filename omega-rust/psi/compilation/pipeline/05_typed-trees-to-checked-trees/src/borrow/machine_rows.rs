//! One machine's borrow rows, built on its own arenas and appended to the
//! program's in roster order.
//!
//! A state's borrow facts read only rows that state wrote: its loan trackers
//! start empty, and every arena lookup goes through a handle it minted. So
//! machines can be built on separate workers, each from fresh arenas. Appending
//! them in roster order reproduces the serial arenas exactly once every handle
//! is moved by the number of rows the earlier machines left in its arena. The
//! destructuring below names every field, so a new handle-bearing field fails
//! to compile here instead of silently keeping a machine-local index.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowCallReferenceSlotEffect, BorrowLoanFact,
    BorrowLoanLineage, BorrowLoanOwnerSegment, BorrowReturnedLoanFact, BorrowWritableRootFact,
    IncomingReferenceIdentity, StateBorrowFact,
};
use facts::PlaceSegment;

use super::state::BorrowFactArenas;

/// Every arena `append_state_borrow_facts` writes, owned by one machine.
#[derive(Default)]
pub(super) struct MachineBorrowRows {
    incoming_references: Arena<IncomingReferenceIdentity>,
    writable_roots: Arena<BorrowWritableRootFact>,
    access_segments: Arena<PlaceSegment>,
    owner_segments: Arena<BorrowLoanOwnerSegment>,
    argument_accesses: Arena<BorrowArgumentAccessFact>,
    calls: Arena<BorrowCallFact>,
    reference_slot_effects: Arena<BorrowCallReferenceSlotEffect>,
    loans: Arena<BorrowLoanFact>,
    loan_suppliers: Arena<Handle<BorrowLoanFact>>,
    returned_loans: Arena<BorrowReturnedLoanFact>,
    states: Arena<StateBorrowFact>,
}

impl MachineBorrowRows {
    pub(super) fn arenas(&mut self) -> BorrowFactArenas<'_> {
        BorrowFactArenas {
            incoming_references: &mut self.incoming_references,
            writable_roots: &mut self.writable_roots,
            access_segments: &mut self.access_segments,
            owner_segments: &mut self.owner_segments,
            argument_accesses: &mut self.argument_accesses,
            calls: &mut self.calls,
            reference_slot_effects: &mut self.reference_slot_effects,
            loans: &mut self.loans,
            loan_suppliers: &mut self.loan_suppliers,
            returned_loans: &mut self.returned_loans,
            states: &mut self.states,
        }
    }

    /// Append these rows after everything already in `program`.
    pub(super) fn append_to(self, program: &mut BorrowFactArenas<'_>) {
        let shift = Shift {
            incoming_references: rows(program.incoming_references),
            writable_roots: rows(program.writable_roots),
            access_segments: rows(program.access_segments),
            owner_segments: rows(program.owner_segments),
            argument_accesses: rows(program.argument_accesses),
            calls: rows(program.calls),
            reference_slot_effects: rows(program.reference_slot_effects),
            loans: rows(program.loans),
            loan_suppliers: rows(program.loan_suppliers),
            returned_loans: rows(program.returned_loans),
        };
        append(
            program.incoming_references,
            self.incoming_references,
            |row| shift.incoming_reference(row),
        );
        append(program.writable_roots, self.writable_roots, |row| row);
        append(program.access_segments, self.access_segments, |row| row);
        append(program.owner_segments, self.owner_segments, |row| row);
        append(program.argument_accesses, self.argument_accesses, |row| {
            shift.argument_access(row)
        });
        append(program.calls, self.calls, |row| shift.call(row));
        append(
            program.reference_slot_effects,
            self.reference_slot_effects,
            |row| shift.reference_slot_effect(row),
        );
        append(program.loans, self.loans, |row| shift.loan(row));
        append(program.loan_suppliers, self.loan_suppliers, |row| {
            shift.handle(row, shift.loans)
        });
        append(program.returned_loans, self.returned_loans, |row| {
            shift.returned_loan(row)
        });
        append(program.states, self.states, |row| shift.state(row));
    }
}

fn rows<T: Default>(arena: &Arena<T>) -> u32 {
    u32::try_from(arena.len()).expect("borrow arena exceeds handle range")
}

fn append<T: Default>(program: &mut Arena<T>, machine: Arena<T>, mut shift: impl FnMut(T) -> T) {
    for row in machine.into_items() {
        program.append(shift(row));
    }
}

/// Rows already in each program arena before this machine's.
struct Shift {
    incoming_references: u32,
    writable_roots: u32,
    access_segments: u32,
    owner_segments: u32,
    argument_accesses: u32,
    calls: u32,
    reference_slot_effects: u32,
    loans: u32,
    loan_suppliers: u32,
    returned_loans: u32,
}

impl Shift {
    /// The invalid handle stays invalid: it means "no row", not row zero.
    fn handle<T>(&self, handle: Handle<T>, rows: u32) -> Handle<T> {
        if !handle.is_valid() {
            return handle;
        }
        Handle::from_parts(
            handle
                .arena_index()
                .checked_add(rows)
                .expect("borrow arena exceeds handle range"),
            handle.generation(),
        )
    }

    fn span<T>(&self, span: HandleSpan<T>, rows: u32) -> HandleSpan<T> {
        if !span.start().is_valid() {
            return span;
        }
        HandleSpan::from_parts(self.handle(span.start(), rows), span.count())
    }

    fn lineage(&self, lineage: BorrowLoanLineage) -> BorrowLoanLineage {
        match lineage {
            BorrowLoanLineage::Reborrow { parent_loan } => BorrowLoanLineage::Reborrow {
                parent_loan: self.handle(parent_loan, self.loans),
            },
            other @ (BorrowLoanLineage::DirectRoot
            | BorrowLoanLineage::UnretainedDerived
            | BorrowLoanLineage::SuppliedOutput
            | BorrowLoanLineage::LifetimeDependency) => other,
        }
    }

    fn incoming_reference(&self, row: IncomingReferenceIdentity) -> IncomingReferenceIdentity {
        let IncomingReferenceIdentity {
            machine_symbol,
            state_symbol,
            origin,
            carrier_type,
            owner_path,
        } = row;
        IncomingReferenceIdentity {
            machine_symbol,
            state_symbol,
            origin,
            carrier_type,
            owner_path: self.span(owner_path, self.owner_segments),
        }
    }

    fn argument_access(&self, row: BorrowArgumentAccessFact) -> BorrowArgumentAccessFact {
        let BorrowArgumentAccessFact {
            expression,
            evaluation_start,
            root,
            segments,
            kind,
            end_before_access,
            released_at,
        } = row;
        BorrowArgumentAccessFact {
            expression,
            evaluation_start,
            root,
            segments: self.span(segments, self.access_segments),
            kind,
            end_before_access,
            released_at,
        }
    }

    fn call(&self, row: BorrowCallFact) -> BorrowCallFact {
        let BorrowCallFact {
            statement_index,
            call_ordinal,
            authored_expression,
            input_reference_loans,
            reference_slot_effects,
            completed_reference_loans,
            receiver_symbol,
            target_symbol,
            has_receiver,
            accesses,
        } = row;
        BorrowCallFact {
            statement_index,
            call_ordinal,
            authored_expression,
            input_reference_loans: input_reference_loans
                .map(|span| self.span(span, self.loan_suppliers)),
            reference_slot_effects: self.span(reference_slot_effects, self.reference_slot_effects),
            completed_reference_loans: self.span(completed_reference_loans, self.loans),
            receiver_symbol,
            target_symbol,
            has_receiver,
            accesses: self.span(accesses, self.argument_accesses),
        }
    }

    fn reference_slot_effect(
        &self,
        row: BorrowCallReferenceSlotEffect,
    ) -> BorrowCallReferenceSlotEffect {
        let BorrowCallReferenceSlotEffect {
            root,
            segments,
            owner_path,
            may_select,
        } = row;
        BorrowCallReferenceSlotEffect {
            root,
            segments: self.span(segments, self.access_segments),
            owner_path: self.span(owner_path, self.owner_segments),
            may_select,
        }
    }

    fn loan(&self, row: BorrowLoanFact) -> BorrowLoanFact {
        let BorrowLoanFact {
            formation,
            incoming_reference,
            incoming_owner_path,
            representation_address,
            statement_index,
            established_by_call,
            formation_expression,
            formation_owner_path,
            retained_operand,
            last_use_statement_index,
            owner_symbol,
            storage_owner,
            owner_path,
            source_owner_symbol,
            supplying_loans,
            retained_input_lifetimes,
            lineage,
            root,
            segments,
            kind,
        } = row;
        BorrowLoanFact {
            formation,
            incoming_reference: self.handle(incoming_reference, self.incoming_references),
            incoming_owner_path: self.span(incoming_owner_path, self.access_segments),
            representation_address: self.span(representation_address, self.access_segments),
            statement_index,
            established_by_call: self.handle(established_by_call, self.calls),
            formation_expression,
            formation_owner_path: formation_owner_path
                .map(|span| self.span(span, self.owner_segments)),
            retained_operand,
            last_use_statement_index,
            owner_symbol,
            storage_owner,
            owner_path: self.span(owner_path, self.owner_segments),
            source_owner_symbol,
            supplying_loans: self.span(supplying_loans, self.loan_suppliers),
            retained_input_lifetimes: retained_input_lifetimes
                .map(|span| self.span(span, self.loan_suppliers)),
            lineage: self.lineage(lineage),
            root,
            segments: self.span(segments, self.access_segments),
            kind,
        }
    }

    fn returned_loan(&self, row: BorrowReturnedLoanFact) -> BorrowReturnedLoanFact {
        let BorrowReturnedLoanFact {
            lineage,
            retained_operand,
            statement_index,
            transition_target,
            formation_expression,
            formation_owner_path,
            incoming_reference,
            incoming_owner_path,
            root,
            segments,
            owner_path,
            kind,
        } = row;
        BorrowReturnedLoanFact {
            lineage: self.lineage(lineage),
            retained_operand,
            statement_index,
            transition_target,
            formation_expression,
            formation_owner_path: formation_owner_path
                .map(|span| self.span(span, self.owner_segments)),
            incoming_reference: self.handle(incoming_reference, self.incoming_references),
            incoming_owner_path: self.span(incoming_owner_path, self.access_segments),
            root,
            segments: self.span(segments, self.access_segments),
            owner_path: self.span(owner_path, self.owner_segments),
            kind,
        }
    }

    fn state(&self, row: StateBorrowFact) -> StateBorrowFact {
        let StateBorrowFact {
            machine_symbol,
            state_symbol,
            incoming_references,
            writable_roots,
            mutable_parameter_count,
            calls,
            loans,
            returned_loans,
        } = row;
        StateBorrowFact {
            machine_symbol,
            state_symbol,
            incoming_references: self.span(incoming_references, self.incoming_references),
            writable_roots: self.span(writable_roots, self.writable_roots),
            mutable_parameter_count,
            calls: self.span(calls, self.calls),
            loans: self.span(loans, self.loans),
            returned_loans: self.span(returned_loans, self.returned_loans),
        }
    }
}
