//! A helper's completed output permits use of the pointer it installed.
//! Keep that event on the saved value, independently of the variable that
//! later holds it. Original input restrictions are constructed separately;
//! they cannot provide the pointer's permission or address.

use arena::Handle;
use checked_trees::{BorrowFacts, BorrowLoanFact, FlowCallFact, FlowFacts, FlowOperandFact};

/// The namespace is established only by this exact call's normal exit.
/// Later uses separately require a current holder or a captured descendant.
pub(crate) fn normal_call(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    loan: Handle<BorrowLoanFact>,
) -> Option<Handle<FlowCallFact>> {
    let source = borrow.supplied_output_call(loan)?;
    let invocation = borrow.calls.get(source);
    let incoming = borrow
        .incoming_references
        .get(borrow.loans.get(loan).incoming_reference);
    let state = flow
        .control
        .unique_state(incoming.machine_symbol, incoming.state_symbol)?;
    let mut calls = state.calls.handles().filter(|handle| {
        let actual = flow.control.calls.get(*handle);
        actual.statement_index == invocation.statement_index
            && actual.call_ordinal == invocation.call_ordinal
            && actual.target_symbol == invocation.target_symbol
            && actual.authored_expression == invocation.authored_expression
            && flow
                .borrow_loan_constraints(actual.exit_constraints)
                .any(|live| live == loan)
    });
    let actual = calls.next()?;
    calls.next().is_none().then_some(actual)
}

/// Read the supplied permission attached to this produced descriptor.
/// This query precedes resource installation: it proves the completed grant,
/// while the resource constructor must still prove every input lifetime and
/// the complete holder/cohort end before the program is admitted.
pub(crate) fn captured_permission(
    program: &dyn validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    capture: &FlowOperandFact,
) -> Result<Handle<BorrowLoanFact>, &'static str> {
    let checked_trees::FlowDescriptorPermission::SuppliedOutput { loan } =
        capture.descriptor_permission
    else {
        return Err("the descriptor has no supplied-output permission");
    };
    if !borrow.loans.is_valid(loan)
        || borrow.loans.get(loan).lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
    {
        return Err("the produced output has no original supplied loan");
    }
    let call = normal_call(borrow, flow, loan)
        .ok_or("the produced output has no exact normal completion")?;
    let invocation = flow.control.calls.get(call);
    if capture.statement_index != invocation.statement_index
        || capture.storage_snapshot != invocation.invocation_storage
    {
        return Err("the produced output belongs to another capture cut");
    }
    let row = borrow.loans.get(loan);
    let incoming = borrow.incoming_references.get(row.incoming_reference);
    let (_, owner) = borrow
        .first_state_borrow(incoming.machine_symbol, incoming.state_symbol)
        .ok_or("the produced output has no borrowing owner")?;
    if !borrow.state_owns_loan(owner, loan) {
        return Err("the produced output carries another state's permission");
    }
    let state = flow
        .control
        .unique_state(incoming.machine_symbol, incoming.state_symbol)
        .ok_or("the produced output has no unique owning state")?;
    if capture.state_symbol != state.state_symbol
        || !state.calls.handles().any(|member| member == call)
    {
        return Err("the produced output permission belongs to another state");
    }
    let path = super::owner_place_segments(
        borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or("the produced output lost its original field path")?,
    );
    let supplied =
        validation::project_type_reference_from_segments(program, incoming.carrier_type, &path)
            .and_then(|reference| validation::reference_shell(program, reference))
            .ok_or("the produced output has no declared reference leaf")?;
    let presented = validation::reference_shell(program, capture.type_reference)
        .ok_or("the produced output capture is not a reference")?;
    if supplied.access != presented.access
        || checked_trees::BorrowAccessKind::from(presented.access) != row.kind
        || !validation::storage_origin_types_match(program, supplied.referee, presented.referee)
    {
        return Err("the produced output differs from its checked reference type or access");
    }
    Ok(loan)
}
