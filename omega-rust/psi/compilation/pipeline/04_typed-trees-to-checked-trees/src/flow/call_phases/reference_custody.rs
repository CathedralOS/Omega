//! Normal invocation completion installs descriptor occurrences before the
//! next operand. The source relation names definite replaced slots and exact
//! transferred permissions. These associations transport existing custody;
//! they do not establish a second local borrow resource. Published local loan
//! lifetimes retain their complete statement-join ledger. Neither a may-write
//! footprint nor a coincident address supplies a transfer.

use super::super::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::{BorrowCallFact, BorrowFacts, FlowConstraintKind, FlowConstraintRef};

pub(in crate::flow) fn install_returned_reference_custody(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext<'_>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    call: &BorrowCallFact,
    binding_writes: Option<&[validation::CanonicalPlace]>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some(handle) = borrow
        .call_at(machine, state, call.statement_index, call.call_ordinal)
        .filter(|handle| borrow.calls.get(*handle) == call)
    else {
        return;
    };
    *constraints = super::super::reference_spans::filter_constraint_refs(
        &mut build.contexts.constraint_refs,
        *constraints,
        |reference| match reference.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                !crate::borrow::calls::replaces_reference_loan(program, borrow, call, loan)
            }
            FlowConstraintKind::BorrowIncomingReference { identity } => {
                incoming_reference_survives_call(program, borrow, call, identity, binding_writes)
            }
            _ => true,
        },
    );
    for loan in call.completed_reference_loans.handles() {
        let value = borrow.loans.get(loan);
        debug_assert_eq!(value.established_by_call, handle);
        super::super::append_constraint_ref(
            &mut build.contexts.constraint_refs,
            constraints,
            FlowConstraintKind::BorrowLoan { loan },
        );
        debug_assert_eq!(value.statement_index, call.statement_index);
    }
}

fn incoming_reference_survives_call(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    call: &BorrowCallFact,
    identity: arena::Handle<checked_trees::IncomingReferenceIdentity>,
    binding_writes: Option<&[validation::CanonicalPlace]>,
) -> bool {
    let incoming = borrow.incoming_references.get(identity);
    // Entry constraints contain the fixed parameter-origin span. Completed
    // invocation/output origins are carried by their exact BorrowLoan rows.
    let checked_trees::IncomingReferenceOrigin::Parameter { symbol } = incoming.origin else {
        return false;
    };
    // An opaque descriptor mutation withdraws current entry
    // custody; missing exact effects cannot mean no replacement.
    // This footprint only retires authority. It never supplies a
    // replacement descriptor or recovers historical snapshots.
    let Some(binding_writes) = binding_writes else {
        return false;
    };
    if call.reference_slot_effects.is_empty() {
        let destination = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: crate::borrow::owner_place_segments(
                borrow.owner_segments.span_or_empty(incoming.owner_path),
            ),
        };
        return !super::super::stored_values::invalidates_destination(
                        program, Some(binding_writes), &destination,
                    );
    }
    !borrow
        .reference_slot_effects
        .span_or_empty(call.reference_slot_effects)
        .iter()
        .any(|effect| {
            !effect.may_select
                && effect.root == facts::PlaceRoot::Symbol(symbol)
                && crate::borrow::place_path_covers_owner(
                    program,
                    borrow.access_segments.span_or_empty(effect.segments),
                    borrow.owner_segments.span_or_empty(incoming.owner_path),
                )
        })
}

/// The successor receives its published joined descriptor roster. Temporary
/// call-transport associations do not mint or dispose local loan resources;
/// their exact call entry/exit constraints authenticate current custody only.
pub(in crate::flow) fn finish_statement_reference_custody(
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext<'_>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    *constraints = super::super::reference_spans::filter_constraint_refs(
        &mut build.contexts.constraint_refs,
        *constraints,
        |reference| match reference.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                !borrow.loans.get(loan).established_by_call.is_valid()
            }
            _ => true,
        },
    );
}

/// The exact normal-call resource row, never a declaration or lifetime
/// source, authenticates this output namespace. Every debt of that same
/// produced descriptor occurrence remains live, even if it has no address.
pub(in crate::flow) fn completed_output_retention(
    borrow: &checked_trees::BorrowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    call: &BorrowCallFact,
    identity: facts::InvocationOutputIdentity,
    carrier_type: typed_trees::types::TypeReferenceHandle,
    output_path: &[checked_trees::BorrowLoanOwnerSegment],
) -> Option<Vec<facts::PlaceRoot>> {
    let performed = borrow
        .call_at(machine, state, statement_index, call.call_ordinal)
        .filter(|handle| borrow.calls.get(*handle) == call)?;
    if identity.state_symbol != state
        || identity.statement_index != statement_index
        || identity.call_ordinal != call.call_ordinal
    {
        return None;
    }
    let mut selected = Vec::new();
    for handle in call.completed_reference_loans.handles() {
        if !borrow.loans.is_valid(handle) {
            return None;
        }
        let loan = borrow.loans.get(handle);
        if loan.established_by_call != performed {
            return None;
        }
        if !loan.incoming_reference.is_valid() {
            continue;
        }
        if !borrow.incoming_references.is_valid(loan.incoming_reference) {
            return None;
        }
        let incoming = borrow.incoming_references.get(loan.incoming_reference);
        if incoming.origin == (checked_trees::IncomingReferenceOrigin::Output { identity }) {
            if borrow.supplied_output_call(handle) != Some(performed) {
                return None;
            }
            if incoming.machine_symbol != machine
                || incoming.state_symbol != state
                || incoming.carrier_type != carrier_type
                || borrow.owner_segments.span_or_empty(incoming.owner_path) != output_path
                || loan.root != facts::PlaceRoot::InvocationOutput(identity)
                || !loan.segments.is_empty()
                || loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
            {
                return None;
            }
            selected.push(loan);
        }
    }
    if selected.is_empty() {
        return None;
    }
    let mut roots = Vec::new();
    for output in selected {
        for handle in call.completed_reference_loans.handles() {
            let loan = borrow.loans.get(handle);
            if loan.storage_owner != output.storage_owner
                || borrow.loan_owner_path(loan) != borrow.loan_owner_path(output)
                || loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
            {
                continue;
            }
            let known = match loan.root {
                facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
                facts::PlaceRoot::Expression(expression) => expression.is_valid(),
                facts::PlaceRoot::InvocationOutput(identity) => {
                    identity.is_valid() && loan.incoming_reference.is_valid()
                }
                facts::PlaceRoot::InvocationReferent { .. } => loan.incoming_reference.is_valid(),
                _ => false,
            };
            if !known {
                return None;
            }
            if !roots.contains(&loan.root) {
                roots.push(loan.root);
            }
        }
    }
    Some(roots)
}
