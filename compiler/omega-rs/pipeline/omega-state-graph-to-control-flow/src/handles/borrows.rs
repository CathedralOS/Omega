use omega_control_flow::{
    StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan,
    StateBorrowWeakening, StateBorrowWritableRoot,
};
use psi_arena::{Handle, HandleSpan};

use super::{remap_handle, remap_span};

pub(crate) fn remap_borrow_writable_root_span(
    roots: HandleSpan<omega_state_graph::StateBorrowWritableRoot>,
) -> HandleSpan<StateBorrowWritableRoot> {
    remap_span(roots)
}

pub(crate) fn remap_borrow_argument_access_span(
    accesses: HandleSpan<omega_state_graph::StateBorrowArgumentAccess>,
) -> HandleSpan<StateBorrowArgumentAccess> {
    remap_span(accesses)
}

pub(crate) fn remap_borrow_call_span(
    calls: HandleSpan<omega_state_graph::StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    remap_span(calls)
}

pub(crate) fn remap_borrow_loan_span(
    loans: HandleSpan<omega_state_graph::StateBorrowLoan>,
) -> HandleSpan<StateBorrowLoan> {
    remap_span(loans)
}

pub(crate) fn remap_borrow_loan_handle(
    loan: Handle<omega_state_graph::StateBorrowLoan>,
) -> Handle<StateBorrowLoan> {
    remap_handle(loan)
}

pub(crate) fn remap_borrow_activation_span(
    activations: HandleSpan<omega_state_graph::StateBorrowActivation>,
) -> HandleSpan<StateBorrowActivation> {
    remap_span(activations)
}

pub(crate) fn remap_borrow_weakening_span(
    weakenings: HandleSpan<omega_state_graph::StateBorrowWeakening>,
) -> HandleSpan<StateBorrowWeakening> {
    remap_span(weakenings)
}
