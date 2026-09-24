use super::filter_expired_borrow_loans;
use checked_trees::{
    BorrowFacts, BorrowLoanFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason,
    FlowConstraintKind, FlowConstraintRef, FlowInvalidationSource,
};

fn single_loan_source(
    borrow: &mut BorrowFacts,
    last_use_statement_index: usize,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
) -> (
    arena::Handle<BorrowLoanFact>,
    arena::HandleSpan<FlowConstraintRef>,
) {
    let loan = borrow.loans.append(BorrowLoanFact {
        last_use_statement_index,
        ..Default::default()
    });
    let source = constraint_refs.insert_many([FlowConstraintRef {
        kind: FlowConstraintKind::BorrowLoan { loan },
    }]);
    (loan, source)
}

fn only_weakening(weakenings: &arena::Arena<FlowBorrowWeakeningFact>) -> &FlowBorrowWeakeningFact {
    let weakenings: Vec<_> = weakenings.iter().map(|(_, fact)| fact).collect();
    let [weakening] = weakenings.as_slice() else {
        panic!("exactly one weakening: {weakenings:?}")
    };
    weakening
}

/// The weakening's recorded boundary is one statement past the loan's
/// recorded last use — a coordinate derived from the loan row, not the
/// position the traversal happened to filter at. A filter pass that reaches
/// an already-expired loan late still stamps the same boundary, so the
/// emitted fact is replayable from the recorded rows alone.
#[test]
fn an_expired_weakening_stamps_the_recorded_loan_boundary() {
    let mut borrow = BorrowFacts::default();
    let mut constraint_refs = arena::Arena::<FlowConstraintRef>::default();
    let (loan, source) = single_loan_source(&mut borrow, 2, &mut constraint_refs);
    let mut weakenings = arena::Arena::<FlowBorrowWeakeningFact>::default();

    // Traversal position 5 is three statements past the recorded expiry
    // boundary; the weakening still stamps `last_use + 1`, not 5.
    let kept = filter_expired_borrow_loans(
        &mut weakenings,
        &mut constraint_refs,
        source,
        &borrow,
        5,
        FlowBorrowWeakeningReason::LastUseExpired,
    );

    assert!(
        kept.is_empty(),
        "the expired loan leaves the constraint set"
    );
    let weakening = only_weakening(&weakenings);
    assert_eq!(
        weakening.source,
        FlowInvalidationSource::Statement { statement_index: 3 },
        "the boundary is the recorded last_use + 1, not the traversal position"
    );
    assert_eq!(weakening.reason, FlowBorrowWeakeningReason::LastUseExpired);
    assert_eq!(weakening.loan, loan);
}

/// At state exit the traversal position is the recorded statement count; a
/// loan whose last use is the final statement stamps that same boundary —
/// `last_use + 1` and the exit position coincide for every loan that can
/// reach the exit filter.
#[test]
fn a_state_exit_weakening_stamps_the_same_recorded_boundary() {
    let mut borrow = BorrowFacts::default();
    let mut constraint_refs = arena::Arena::<FlowConstraintRef>::default();
    let (loan, source) = single_loan_source(&mut borrow, 3, &mut constraint_refs);
    let mut weakenings = arena::Arena::<FlowBorrowWeakeningFact>::default();

    // Statement count 4: the loan's last use is the final statement index.
    let kept = filter_expired_borrow_loans(
        &mut weakenings,
        &mut constraint_refs,
        source,
        &borrow,
        4,
        FlowBorrowWeakeningReason::StateExit,
    );

    assert!(kept.is_empty());
    let weakening = only_weakening(&weakenings);
    assert_eq!(
        weakening.source,
        FlowInvalidationSource::Statement { statement_index: 4 }
    );
    assert_eq!(weakening.reason, FlowBorrowWeakeningReason::StateExit);
    assert_eq!(weakening.loan, loan);
}

/// A loan still live at the traversal position keeps its constraint and
/// records no weakening.
#[test]
fn a_live_loan_keeps_its_constraint_without_a_weakening() {
    let mut borrow = BorrowFacts::default();
    let mut constraint_refs = arena::Arena::<FlowConstraintRef>::default();
    let (_, source) = single_loan_source(&mut borrow, 5, &mut constraint_refs);
    let mut weakenings = arena::Arena::<FlowBorrowWeakeningFact>::default();

    let kept = filter_expired_borrow_loans(
        &mut weakenings,
        &mut constraint_refs,
        source,
        &borrow,
        3,
        FlowBorrowWeakeningReason::LastUseExpired,
    );

    assert_eq!(kept.count(), 1);
    assert_eq!(weakenings.len(), 0);
}
