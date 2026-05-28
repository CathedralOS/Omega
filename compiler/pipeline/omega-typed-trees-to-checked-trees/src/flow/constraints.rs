use super::*;

pub(super) fn append_contiguous_borrow_root_constraints(
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    refs: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    roots: omega_core::arena::HandleSpan<BorrowWritableRootFact>,
) {
    let start = roots.start();
    if !start.is_valid() {
        return;
    }

    for offset in 0..roots.count() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowWritableRoot {
                root: Handle::from_parts(start.arena_index() + offset, start.generation()),
            },
        );
    }
}

pub(super) fn append_contiguous_borrow_access_constraints(
    constraint_refs: &mut omega_core::arena::Arena<FlowConstraintRef>,
    refs: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    let start = accesses.start();
    if !start.is_valid() {
        return;
    }

    for offset in 0..accesses.count() {
        append_constraint_ref(
            constraint_refs,
            refs,
            FlowConstraintKind::BorrowAccess {
                access: Handle::from_parts(start.arena_index() + offset, start.generation()),
            },
        );
    }
}
