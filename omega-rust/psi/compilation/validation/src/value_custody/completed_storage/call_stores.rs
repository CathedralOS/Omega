//! Find the reference generation installed by a helper that actually returned.
//! The original call, its published loans and its completed storage must agree.
//! This identifies a stored value; its pointer permission and lifetime remain
//! separate obligations for the consuming operation.

use arena::Handle;
use checked_trees::{BorrowLoanFact, BorrowLoanFormation, CheckFacts, FlowCallFact};
use symbols::SymbolHandle;

pub struct CompletedCallDescriptor {
    pub call: Handle<FlowCallFact>,
    pub loan: Handle<BorrowLoanFact>,
    pub captured: super::StoredCapture,
}

/// Statement continuation can retain several call generations. Resolve every
/// original occurrence, preserving their identities and refusing an incomplete
/// or cyclic supplier roster instead of choosing a convenient same-home call.
pub fn loan_call_descriptors(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    loan: Handle<BorrowLoanFact>,
) -> Result<Option<Vec<CompletedCallDescriptor>>, &'static str> {
    let borrow = &facts.borrow;
    let owner = borrow
        .unique_state_borrow(machine, state)
        .ok_or("the installed reference has no unique borrowing state")?;
    if !borrow.state_owns_loan(owner, loan) {
        return Err("the installed reference belongs to another borrowing state");
    }
    if borrow.loans.get(loan).formation != BorrowLoanFormation::ReturnedDescriptor {
        return Ok(None);
    }
    let flow = &facts.flow;
    let control = &flow.control;
    let flow_state = control
        .unique_state(machine, state)
        .ok_or("the installed reference has no unique flow state")?;
    let mut pending = vec![(loan, false)];
    let mut active = Vec::new();
    let mut completed = Vec::new();
    let mut output = Vec::new();
    while let Some((handle, closing)) = pending.pop() {
        if closing {
            active.pop();
            completed.push(handle);
            continue;
        }
        if completed.contains(&handle) {
            continue;
        }
        if active.contains(&handle) || !borrow.state_owns_loan(owner, handle) {
            return Err("the installed reference has a cyclic or foreign supplier");
        }
        let row = borrow.loans.get(handle);
        if row.formation != BorrowLoanFormation::ReturnedDescriptor {
            return Err("the installed reference supplier is not a call generation");
        }
        active.push(handle);
        pending.push((handle, true));
        if !row.established_by_call.is_valid() {
            let suppliers = borrow
                .loan_suppliers
                .span(row.supplying_loans)
                .filter(|suppliers| !suppliers.is_empty())
                .ok_or("the installed reference lost its original call generation")?;
            for supplier in suppliers.iter().rev() {
                pending.push((*supplier, false));
            }
            continue;
        }
        if !owner
            .calls
            .handles()
            .any(|call| call == row.established_by_call)
            || !borrow.calls.is_valid(row.established_by_call)
        {
            return Err("the installed reference names another state's call");
        }
        let source = borrow.calls.get(row.established_by_call);
        if source.statement_index != row.statement_index
            || !source
                .completed_reference_loans
                .handles()
                .any(|loan| loan == handle)
        {
            return Err("the installed reference is not in its call's completed roster");
        }
        let mut calls = flow_state.calls.handles().filter(|call| {
            let call = control.calls.get(*call);
            call.statement_index == source.statement_index
                && call.call_ordinal == source.call_ordinal
                && call.target_symbol == source.target_symbol
                && call.authored_expression == source.authored_expression
                && flow
                    .borrow_loan_constraints(call.exit_constraints)
                    .any(|loan| loan == handle)
        });
        let call = calls
            .next()
            .ok_or("the installed reference has no performed normal completion")?;
        if calls.next().is_some() {
            return Err("the installed reference has ambiguous normal completions");
        }
        let invocation = control.calls.get(call);
        let destination = crate::CanonicalPlace {
            root: row.storage_owner,
            segments: super::loan_homes::place_segments(borrow.loan_owner_path(row)),
        };
        let captures = super::at_snapshot(
            program,
            control,
            invocation.completed_storage,
            &destination,
            super::StoredObservation::Binding,
        )
        .and_then(super::StoredOperand::captures)
        .filter(|captures| !captures.is_empty())
        .ok_or("the completed call has no complete installed descriptor frontier")?;
        for mut captured in captures {
            let value = control.operands.get(captured.capture);
            if value.state_symbol != state
                || value.statement_index != row.statement_index
                || value.storage_snapshot != invocation.invocation_storage
            {
                return Err("the installed descriptor belongs to another call cut");
            }
            captured.projection = crate::corresponding_place_projection(
                program,
                captured.reference,
                &captured.projection,
            )
            .ok_or("the installed descriptor lost its applied member correspondence")?;
            let reference = crate::project_type_reference_from_segments(
                program,
                captured.reference,
                &captured.projection,
            )
            .and_then(|reference| crate::reference_shell(program, reference))
            .ok_or("the completed store does not contain the declared reference")?;
            // A debt can retain stronger input access than the pointer stored
            // beside it. Both share this installation, but only the pointer
            // row's kind describes its physical reference access.
            if row.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                && checked_trees::BorrowAccessKind::from(reference.access)
                    .direct_reborrow_effect(&row.kind)
                    .is_none()
            {
                return Err("the installed descriptor cannot supply its recorded access");
            }
            output.push(CompletedCallDescriptor {
                call,
                loan: handle,
                captured,
            });
        }
    }
    Ok(Some(output))
}
