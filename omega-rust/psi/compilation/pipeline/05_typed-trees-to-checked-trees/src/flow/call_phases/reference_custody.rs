//! Normal completion makes returned permissions and slot replacements current
//! before the next operand. Returned values keep their ordinary loan lifetime;
//! temporary slot associations last through this invocation's transport only.
//! Exact source relationships own both roles; equal addresses and may-write
//! footprints cannot supply either permission or a replacement value.

use super::super::FlowBuildContext;
use super::super::append_constraint_ref;
use super::super::reference_spans::filter_constraint_refs;
use super::super::stored_values::invalidates_destination;
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
    *constraints = filter_constraint_refs(
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
        append_constraint_ref(
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
    // Ordinary calls may replace descriptors stored inside borrowed data, but
    // cannot replace the root parameter loan supplied by the enclosing caller.
    // Its lifetime remains that incomplete invocation, even after a write.
    if matches!(
        incoming.origin,
        checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
    ) {
        return true;
    }
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
        return !invalidates_destination(program, Some(binding_writes), &destination);
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

/// Remove only temporary slot-transport associations. A returned value's
/// permission and input lifetimes remain until their actual holders end;
/// named bindings have already retained the same evaluated result.
pub(in crate::flow) fn finish_statement_reference_custody(
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext<'_>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    *constraints = filter_constraint_refs(
        &mut build.contexts.constraint_refs,
        *constraints,
        |reference| match reference.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let row = borrow.loans.get(loan);
                !row.established_by_call.is_valid() || borrow.is_call_result_loan(row)
            }
            _ => true,
        },
    );
}
