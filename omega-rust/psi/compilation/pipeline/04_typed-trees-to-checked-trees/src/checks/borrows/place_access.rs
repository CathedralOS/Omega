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
    !backings.iter().all(|backing| {
        if backing.retains_loan(borrow, loan_handle) {
            return true;
        }
        // Distinct leaves returned together share an input lifetime, not
        // storage authority. Through another leaf of that same call compare
        // the actual referents; overlapping exclusive leaves still conflict.
        // External accesses and ancestor aliases keep the full input held.
        let shares_call = loan.retained_operand.is_some_and(|retained| {
            retained.formation_expression.is_valid()
                && backing.lineage.iter().any(|ancestor| {
                    let ancestor = borrow.loans.get(*ancestor);
                    ancestor.root == loan.root
                        && ancestor.incoming_reference == loan.incoming_reference
                        && ancestor.retained_operand == Some(retained)
                        // A parent and child inherit the same occurrence too.
                        // Only disjoint descriptor referents are peers; a
                        // projected access through the parent cannot fake it.
                        && captured_place_compatibility(
                            program,
                            &CapturedPlace {
                                incoming_reference: ancestor.incoming_reference,
                                root: ancestor.root,
                                segments: borrow.loan_segments(ancestor).to_vec(),
                            },
                            &ancestor.kind,
                            &CapturedPlace {
                                incoming_reference: loan.incoming_reference,
                                root: loan.root,
                                segments: borrow.loan_segments(loan).to_vec(),
                            },
                            &loan.kind,
                            premises,
                            bound_lookup,
                        ).disjoint
                })
        });
        let segments = borrow.loan_segments(loan);
        let depth = loan
            .retained_operand
            .filter(|_| !shares_call)
            .map_or(segments.len(), |retained| retained.depth);
        let loan_place = CapturedPlace {
            incoming_reference: loan.incoming_reference,
            root: loan.root,
            segments: segments[..depth].to_vec(),
        };
        captured_place_compatibility(
            program,
            &backing.place,
            kind,
            &loan_place,
            &loan.kind,
            premises,
            bound_lookup,
        )
        .non_interfering
    })
}
