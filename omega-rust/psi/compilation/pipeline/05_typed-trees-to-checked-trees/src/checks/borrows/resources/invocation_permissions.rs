//! A provider result uses the permission established by its completed call.
//! Input lifetimes keep backing alive but do not identify the returned address.

use arena::Handle;
use checked_trees::{BorrowLoanFact, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};

/// Checked bodies retain their checked source relation. Opaque results instead
/// require the exact permission published at this invocation's normal return.
pub(in crate::checks::borrows) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    invocation: &FlowCallFact,
    capture: &FlowOperandFact,
    result_type: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<Result<Handle<BorrowLoanFact>, &'static str>> {
    if validation::call_target_has_checked_body_contract(program, invocation.target_symbol) {
        return None;
    }
    Some(resolve_opaque(
        program,
        facts,
        state,
        invocation,
        capture,
        result_type,
        path,
    ))
}

fn resolve_opaque(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    invocation: &FlowCallFact,
    capture: &FlowOperandFact,
    result_type: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Result<Handle<BorrowLoanFact>, &'static str> {
    if capture.state_symbol != state.state_symbol
        || capture.statement_index != invocation.statement_index
        || !invocation.authored_expression.is_valid()
    {
        return Err("the returned permission belongs to another invocation");
    }
    let call = facts
        .borrow
        .call_at(
            state.machine_symbol,
            state.state_symbol,
            invocation.statement_index,
            invocation.call_ordinal,
        )
        .ok_or("the completed invocation has no checked borrow record")?;
    let checked = facts.borrow.calls.get(call);
    if checked.authored_expression != invocation.authored_expression
        || validation::call_target_entry_symbol(program, checked.target_symbol)
            != validation::call_target_entry_symbol(program, invocation.target_symbol)
    {
        return Err("the returned permission disagrees with its performed call");
    }
    let selected = validation::project_type_reference_from_segments(program, result_type, path)
        .ok_or("the returned permission has no declared result field")?;
    let access = crate::borrow::reference_borrow_access_kind(program, selected)
        .ok_or("the returned permission does not name a reference field")?;
    let expected = validation::reference_shell(program, selected)
        .ok_or("the returned permission has no reference carrier")?;
    let mut permissions = checked.completed_reference_loans.handles().filter(|loan| {
        let row = facts.borrow.loans.get(*loan);
        let Some(formed) = row.formation_owner_path else {
            return false;
        };
        if row.lineage != checked_trees::BorrowLoanLineage::DirectRoot
            || row.formation != checked_trees::BorrowLoanFormation::ReturnedDescriptor
            || row.established_by_call != call
            || row.statement_index != invocation.statement_index
            || row.formation_expression != invocation.authored_expression
            || row.root != facts::PlaceRoot::Expression(invocation.authored_expression)
            || row.kind.direct_reborrow_effect(&access).is_none()
            || !facts
                .borrow
                .incoming_references
                .is_valid(row.incoming_reference)
            || !facts
                .flow
                .borrow_loan_constraints(capture.constraints)
                .any(|active| active == *loan)
        {
            return false;
        }
        let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
        let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } =
            incoming.origin
        else {
            return false;
        };
        let owner = facts.borrow.owner_segments.span_or_empty(formed);
        let projection = crate::borrow::owner_place_segments(owner);
        incoming.machine_symbol == state.machine_symbol
            && incoming.state_symbol == state.state_symbol
            && expression == invocation.authored_expression
            && validation::call_target_entry_symbol(program, target)
                == validation::call_target_entry_symbol(program, invocation.target_symbol)
            && validation::reference_shell(program, incoming.carrier_type).is_some_and(|supplied| {
                program.normalized_type_identity(supplied.referee)
                    == program.normalized_type_identity(expected.referee)
            })
            && facts
                .borrow
                .owner_segments
                .span_or_empty(incoming.owner_path)
                == owner
            && projection.len() == path.len()
            && projection.iter().zip(path).all(|(left, right)| {
                crate::flow::canonical_place_segments_equal(program, *left, *right)
            })
    });
    let Some(permission) = permissions.next() else {
        if ::validation::trace_flags::state_reference_diagnostics() {
            eprintln!(
                "returned-reference-permission-refused call={call:?} expression={:?} path={path:?} expected_type={selected:?} expected_access={access:?} capture={capture:?}",
                invocation.authored_expression,
            );
            for loan in checked.completed_reference_loans.handles().take(12) {
                let row = facts.borrow.loans.get(loan);
                let incoming = facts
                    .borrow
                    .incoming_references
                    .is_valid(row.incoming_reference)
                    .then(|| facts.borrow.incoming_references.get(row.incoming_reference));
                eprintln!(
                    "returned-reference-permission-candidate loan={loan:?} row={row:?} incoming={incoming:?}"
                );
            }
        }
        return Err("the completed result has no active permission for this reference field");
    };
    if permissions.next().is_some() {
        return Err("the completed result has ambiguous reference permissions");
    }
    Ok(permission)
}
