use omega_control_flow::{
    ContainedFlow, MachineOwnedDataFlow, Operation, StateBorrowActivation,
    StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan, StateBorrowWeakening,
    StateBorrowWritableRoot, StateContractCall, StateContractExit, StateContractFactRef,
    StateDropEvent, StateFlow, StateMoveEvent, StateParameterFlow, TransitionFlow,
};
use omega_core::arena::{Handle, HandleSpan};

fn remap_handle<From, To>(handle: Handle<From>) -> Handle<To> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_span<From, To>(span: HandleSpan<From>) -> HandleSpan<To> {
    HandleSpan::from_parts(remap_handle(span.start()), span.count())
}

pub(crate) fn remap_contained_span(
    contained: HandleSpan<omega_state_graph::ContainedGraph>,
) -> HandleSpan<ContainedFlow> {
    remap_span(contained)
}

pub(crate) fn remap_owned_data_span(
    owned_data: HandleSpan<omega_state_graph::MachineOwnedDataGraph>,
) -> HandleSpan<MachineOwnedDataFlow> {
    remap_span(owned_data)
}

pub(crate) fn remap_parameter_span(
    parameters: HandleSpan<omega_state_graph::StateParameterNode>,
) -> HandleSpan<StateParameterFlow> {
    remap_span(parameters)
}

pub(crate) fn remap_state_span(
    states: HandleSpan<omega_state_graph::StateNode>,
) -> HandleSpan<StateFlow> {
    remap_span(states)
}

pub(crate) fn remap_operation_span(
    operations: HandleSpan<omega_state_graph::Operation>,
) -> HandleSpan<Operation> {
    remap_span(operations)
}

pub(crate) fn remap_transition_span(
    transitions: HandleSpan<omega_state_graph::TransitionEdge>,
) -> HandleSpan<TransitionFlow> {
    remap_span(transitions)
}

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

pub(crate) fn remap_move_event_span(
    moves: HandleSpan<omega_state_graph::StateMoveEvent>,
) -> HandleSpan<StateMoveEvent> {
    remap_span(moves)
}

pub(crate) fn remap_drop_event_span(
    drops: HandleSpan<omega_state_graph::StateDropEvent>,
) -> HandleSpan<StateDropEvent> {
    remap_span(drops)
}

pub(crate) fn remap_contract_fact_ref_span(
    refs: HandleSpan<omega_state_graph::StateContractFactRef>,
) -> HandleSpan<StateContractFactRef> {
    remap_span(refs)
}

pub(crate) fn remap_contract_call_span(
    calls: HandleSpan<omega_state_graph::StateContractCall>,
) -> HandleSpan<StateContractCall> {
    remap_span(calls)
}

pub(crate) fn remap_contract_exit_span(
    exits: HandleSpan<omega_state_graph::StateContractExit>,
) -> HandleSpan<StateContractExit> {
    remap_span(exits)
}

pub(crate) fn remap_expression_span(
    expressions: HandleSpan<omega_checked_trees::expression::ExpressionHandle>,
) -> HandleSpan<omega_checked_trees::expression::ExpressionHandle> {
    remap_span(expressions)
}
