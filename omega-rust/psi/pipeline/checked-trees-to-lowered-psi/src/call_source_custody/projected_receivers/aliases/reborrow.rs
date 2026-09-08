use super::*;

pub(super) fn reborrow_resource(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    loan_handle: Handle<BorrowLoanFact>,
    parent_handle: Handle<BorrowLoanFact>,
    owner: SymbolHandle,
    activation: FlowInvalidationSource,
    weakening: FlowInvalidationSource,
    reason: FlowBorrowWeakeningReason,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let loan = borrow.loans.get(loan_handle);
    let parents = borrow
        .direct_loan_resources
        .iter()
        .filter(|(_, row)| row.loan == parent_handle)
        .map(|(resource, _)| CheckedParentBorrowResource::DirectRoot { resource })
        .chain(
            borrow
                .reborrow_loan_resources
                .iter()
                .filter(|(_, row)| row.loan == parent_handle)
                .map(|(resource, _)| CheckedParentBorrowResource::Reborrow { resource }),
        )
        .collect::<Vec<_>>();
    let [parent] = parents.as_slice() else {
        return unsupported("receiver alias has no unique immediate parent resource");
    };
    let resources = borrow
        .reborrow_loan_resources
        .iter()
        .filter(|(_, row)| row.loan == loan_handle)
        .collect::<Vec<_>>();
    let [(_, resource)] = resources.as_slice() else {
        return unsupported("receiver alias has no unique reborrow resource");
    };
    if resource.machine_symbol != machine
        || resource.state_symbol != state
        || resource.owner_symbol != owner
        || !resource.owner_path.is_empty()
        || resource.captured_place.root_symbol != loan.root_symbol
        || resource.captured_place.segments != borrow.loan_segments(loan)
        || resource.access != BorrowAccessKind::WriteOnly
        || resource.parent_access != BorrowAccessKind::WriteOnly
        || resource.parent_loan != parent_handle
        || resource.parent_resource != *parent
        || resource.activation_source != activation
        || resource.weakening_source != weakening
        || resource.weakening_reason != reason
    {
        return unsupported("receiver alias reborrow disagrees with its source lifetime");
    }
    let suspension = &resource.parent_suspension;
    let lifetimes = &checked.facts.flow.borrow_lifetimes;
    let child_activation = lifetimes.activations.get(suspension.child_activation);
    let child_weakening = lifetimes
        .weakenings
        .get(resource.parent_end_status.child_weakening);
    let parent_weakening = lifetimes
        .weakenings
        .get(resource.parent_end_status.parent_weakening);
    let flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, flow)] = flow_states.as_slice() else {
        return unsupported("receiver alias formation has no unique flow state");
    };
    let entries = checked
        .facts
        .flow
        .control
        .statements
        .span_or_empty(flow.statements)
        .iter()
        .filter(|row| row.statement_index == loan.statement_index)
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return unsupported("receiver alias formation has no unique entry");
    };
    let constraint = checked
        .facts
        .flow
        .contexts
        .constraint_refs
        .get(suspension.parent_entry_constraint);
    if !contains_handle(flow.borrow_activations, suspension.child_activation)
        || !contains_handle(
            flow.borrow_weakenings,
            resource.parent_end_status.child_weakening,
        )
        || !contains_handle(
            flow.borrow_weakenings,
            resource.parent_end_status.parent_weakening,
        )
        || !contains_handle(entry.entry_constraints, suspension.parent_entry_constraint)
        || !lifetimes
            .activations
            .span_or_empty(flow.borrow_activations)
            .contains(child_activation)
        || !lifetimes
            .weakenings
            .span_or_empty(flow.borrow_weakenings)
            .contains(child_weakening)
        || !lifetimes
            .weakenings
            .span_or_empty(flow.borrow_weakenings)
            .contains(parent_weakening)
        || !checked
            .facts
            .flow
            .contexts
            .constraint_refs
            .span_or_empty(entry.entry_constraints)
            .contains(constraint)
        || !matches!(constraint.kind, FlowConstraintKind::BorrowLoan { loan } if loan == parent_handle)
        || child_activation.loan != loan_handle
        || child_activation.source != activation
        || child_weakening.loan != loan_handle
        || child_weakening.source != weakening
        || child_weakening.reason != reason
        || parent_weakening.loan != parent_handle
        || suspension.child_loan != loan_handle
        || suspension.parent_loan != parent_handle
        || suspension.parent_resource != *parent
        || suspension.source != activation
    {
        return unsupported(
            "receiver alias formation or lifetime is outside its exact flow roster",
        );
    }
    // The automatic root-handoff receiving pass independently checks each
    // containment certificate, parent status, restoration and final disposition.
    // Source replay binds those resources to immediate authored initializers;
    // it never interprets lexical parent retirement as restored authority.
    Ok(())
}

fn contains_handle<T>(span: arena::HandleSpan<T>, handle: Handle<T>) -> bool {
    handle.is_valid()
        && handle.generation() == span.start().generation()
        && handle
            .arena_index()
            .checked_sub(span.start().arena_index())
            .is_some_and(|offset| offset < span.count())
}
