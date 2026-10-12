//! One live-loan judgment for every access to storage.
//!
//! Reads, writes, call operands and exclusive operations name a place and an
//! access kind. A place rooted at a local reference that owns a live loan
//! denotes the storage that loan captured, not the local's name, so the
//! access is first rejoined to every backing it may denote. Storage identity
//! and authority are separate judgments: each backing is compared with every
//! live loan, and only the access's own retained lineage is exempt. A child
//! formed through the accessing alias is not part of that lineage, so an
//! exclusive child suspends the branch it captured for any other access until
//! its last use, and a shared child freezes that branch's mutation.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowAccessKind, BorrowFacts, BorrowLoanFact, CapturedPlace, CheckFacts, FlowConstraintRef,
    FlowStateFact,
};

use language_semantics::ReferenceAccess;
use typed_trees::expression::ExpressionNode;

use super::access_sources;
use super::aliases::ResolvedAlias;
use super::overlap::{StatedOrderingPremise, captured_place_compatibility};

/// Every backing an access of `place` may denote, with the retained lineage
/// that grants its authority. A root that owns none of the live loans (a
/// parameter, an owner, a reference initialized from static text) names its
/// own storage. A local reference that owns one is rejoined to the storage
/// its loans captured; `None` when that backing cannot be established.
pub(in crate::checks::borrows) fn place_backings(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    entry_constraints: HandleSpan<FlowConstraintRef>,
    mut live_loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    if !live_loans.any(|loan| facts.borrow.loans.get(loan).storage_owner == place.root) {
        return Some(vec![ResolvedAlias {
            place,
            lineage: Vec::new(),
        }]);
    }
    access_sources::resolve_captured_all(
        program,
        facts,
        state,
        statement_index,
        entry_constraints,
        place,
    )
}

/// Whether an access of `kind` through `backings` interferes with the live
/// loan `loan_handle`.
pub(in crate::checks::borrows) fn access_interferes<'p>(
    program: &'p dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    backings: &[ResolvedAlias],
    kind: &BorrowAccessKind,
    loan_handle: Handle<BorrowLoanFact>,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    let loan = borrow.loans.get(loan_handle);
    let loan_place = CapturedPlace {
        incoming_reference: loan.incoming_reference,
        root: loan.root,
        segments: borrow.loan_segments(loan).to_vec(),
    };
    // A call's still-live operands stay loaned while a returned reference is
    // live, so the result loan's custody covers each operand place it could
    // derive from, not only the projected leaf recorded as `lent`.
    let covered = call_result_operand_places(program, loan, loan_place);
    !backings.iter().all(|backing| {
        backing.lineage.contains(&loan_handle)
            || covered.iter().all(|loan_place| {
                captured_place_compatibility(
                    program,
                    &backing.place,
                    kind,
                    loan_place,
                    &loan.kind,
                    premises,
                    bound_lookup,
                )
                .non_interfering
            })
    })
}

/// The receiver places a call's exclusively borrowed operands name, for a
/// loan produced by binding that call's returned reference. The result's
/// `lent` records the selected leaf for origin identity; custody instead
/// keeps each exclusive operand's receiver, so its coverage is the operand's
/// root alone. A shared `&` operand borrows only the descriptor it spells —
/// its referent is not the callee's receiver — and non-borrow operands carry
/// no loan, so neither extends the result's custody.
fn call_result_operand_places(
    program: &dyn ::validation::CheckingInput,
    loan: &BorrowLoanFact,
    loan_place: CapturedPlace,
) -> Vec<CapturedPlace> {
    let mut covered = vec![loan_place];
    let ExpressionNode::Call(call) = program
        .expression_table
        .expression(loan.formation_expression)
    else {
        return covered;
    };
    let mut operands = program
        .expression_table
        .expression_handles(call.arguments)
        .to_vec();
    if call.receiver.is_valid() {
        operands.push(call.receiver);
    }
    for operand in operands {
        if operand != call.receiver {
            match program.expression_table.expression(operand) {
                ExpressionNode::Borrow(borrow)
                    if borrow.access != ReferenceAccess::Shared => {}
                _ => continue,
            }
        }
        let Some(place) = validation::canonical_place_from_expression(program, operand)
        else {
            continue;
        };
        let place = CapturedPlace {
            incoming_reference: Default::default(),
            root: place.root,
            segments: Vec::new(),
        };
        if !covered.contains(&place) {
            covered.push(place);
        }
    }
    covered
}
