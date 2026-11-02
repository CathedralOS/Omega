//! Append one constraint ref per entry of a contiguous borrow-fact span: each
//! writable root of a state, or each argument access of a call.
use crate::flow::append_constraint_ref;
use checked_trees::{
    BorrowArgumentAccessFact, BorrowWritableRootFact, FlowConstraintKind, FlowConstraintRef,
};

pub(super) fn append_contiguous_borrow_root_constraints(
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    refs: &mut arena::HandleSpan<FlowConstraintRef>,
    roots: arena::HandleSpan<BorrowWritableRootFact>,
) {
    let start = roots.start();
    if !start.is_valid() {
        return;
    }

    for root in roots.handles() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowWritableRoot { root },
        );
    }
}

pub(super) fn append_contiguous_borrow_access_constraints(
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    refs: &mut arena::HandleSpan<FlowConstraintRef>,
    accesses: arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    let start = accesses.start();
    if !start.is_valid() {
        return;
    }

    for access in accesses.handles() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowAccess { access },
        );
    }
}
