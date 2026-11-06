//! A reference saved by a helper keeps the evaluated input that lent it.
//! Bind the completed stored generation to those original call resources.
//! Their full access and parent lifetime stay intact even when the saved
//! pointer selects a smaller part of the input's data.

use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use arena::Handle;
use checked_trees::{BorrowLoanLineage, CheckFacts, CheckedCallBorrowLoanSource};

pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    facts.borrow.stored_call_inputs.reset_retain_capacity();
    facts
        .borrow
        .stored_call_input_resources
        .reset_retain_capacity();
    let prepared_loans = LoanOccurrences::new(program, facts);
    let loan_occurrences = &prepared_loans;
    let mut retained = Vec::new();
    for (_, state) in facts.flow.control.states.iter() {
        let Some(owner) = facts
            .borrow
            .unique_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            continue;
        };
        for (loan, row) in facts.borrow.state_loans(owner) {
            if row.lineage != BorrowLoanLineage::UnretainedDerived
                || !row.established_by_call.is_valid()
            {
                continue;
            }
            match retained_inputs(program, facts, loan_occurrences, state, loan, row) {
                Ok(inputs) => retained.push((loan, inputs)),
                Err(reason) if ::validation::trace_flags::check_sites() => {
                    eprintln!(
                        "CHECKSITE stored-call-input-unavailable loan={loan:?} reason={reason}"
                    );
                }
                Err(_) => {}
            }
        }
    }
    drop(prepared_loans);
    for (loan, inputs) in retained {
        let resources = facts.borrow.stored_call_input_resources.insert_many(inputs);
        facts
            .borrow
            .stored_call_inputs
            .insert(checked_trees::CheckedStoredCallBorrowInputs { loan, resources });
    }
}

fn retained_inputs(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &checked_trees::FlowStateFact,
    loan: Handle<checked_trees::BorrowLoanFact>,
    row: &checked_trees::BorrowLoanFact,
) -> Result<Vec<Handle<checked_trees::CheckedCallBorrowLoanResource>>, &'static str> {
    let descriptors = validation::completed_storage::loan_call_descriptors(
        program,
        facts,
        state.machine_symbol,
        state.state_symbol,
        loan,
    )?
    .ok_or("the saved reference has no completed call generation")?;
    let mut inputs = Vec::new();
    for descriptor in descriptors {
        let validation::completed_storage::CompletedCallDescriptorSource::Captured(source) =
            descriptor.source
        else {
            return Err("the saved reference still requires its original incoming permission");
        };
        let (suppliers, source_parents) = super::permission_sources::lending_sources(
            program,
            facts,
            loan_occurrences,
            state,
            descriptor.call,
            source.capture,
            &source.projection,
            &row.kind,
        )?;
        // Parent handles below belong to this collector's scratch
        // arena. Compare their complete roster, never republish it.
        for supplier in suppliers {
            let original = match supplier.source {
                CheckedCallBorrowLoanSource::CallAccess { .. } => {
                    if !facts.flow.control.operands.is_valid(supplier.actual) {
                        return Err("the saved reference lost its original evaluated input");
                    }
                    crate::checks::borrows::formation_captures::actual_invocation(
                        program,
                        facts,
                        state,
                        supplier.actual,
                    )?
                    .ok_or("the saved reference has no original invocation tuple")?
                }
                CheckedCallBorrowLoanSource::StoredLoan { .. } => descriptor.call,
            };
            let mut found = false;
            for (resource_handle, resource) in facts.borrow.call_loan_resources.iter() {
                if resource.call != original
                    || resource.parent_lifetime.root != supplier.backing.root
                    || resource.access.direct_reborrow_effect(&row.kind).is_none()
                {
                    continue;
                }
                let same_permission = match (&supplier.source, &resource.source) {
                    (
                        CheckedCallBorrowLoanSource::StoredLoan { loan },
                        CheckedCallBorrowLoanSource::StoredLoan { loan: original },
                    ) => loan == original,
                    (
                        CheckedCallBorrowLoanSource::CallAccess { access, parents },
                        CheckedCallBorrowLoanSource::CallAccess {
                            access: original,
                            parents: original_parents,
                        },
                    ) => {
                        resource.operand == supplier.actual
                            && access == original
                            && facts.borrow.call_loan_parents.span(*original_parents)
                                == source_parents.span(*parents)
                    }
                    _ => false,
                };
                if !same_permission {
                    continue;
                }
                found = true;
                if !inputs.contains(&resource_handle) {
                    inputs.push(resource_handle);
                }
            }
            if !found {
                return Err("the saved reference's original call resource is unavailable");
            }
        }
    }
    if inputs.is_empty() {
        return Err("the saved reference has no complete input resource roster");
    }
    Ok(inputs)
}
