//! One completed call may record the same input restriction on several outputs.
//! Keep each output holder while comparing their exact captured input identity.

use arena::Handle;
use checked_trees::{BorrowLoanFact, BorrowLoanLineage, CheckFacts, FlowStateFact};

use super::same_carrier_generation;

/// Several returned fields can retain the same input restriction. They keep
/// separate output paths, but recording that one captured input twice does not
/// borrow it twice. This applies only within one performed completion; neither
/// a different input nor a later call can use the exemption.
pub(in crate::checks::borrows) fn same_completed_input_debt(
    facts: &CheckFacts,
    state: &FlowStateFact,
    left_handle: Handle<BorrowLoanFact>,
    right_handle: Handle<BorrowLoanFact>,
) -> bool {
    let borrow = &facts.borrow;
    let left = borrow.loans.get(left_handle);
    let right = borrow.loans.get(right_handle);
    let Some(parameter) = left.retained_input_parameter else {
        return false;
    };
    if left.lineage != BorrowLoanLineage::LifetimeDependency
        || right.lineage != BorrowLoanLineage::LifetimeDependency
        || !same_carrier_generation(left, right)
        || !borrow.calls.is_valid(left.established_by_call)
        || !left.formation_expression.is_valid()
        || left.formation_expression != right.formation_expression
        || left.retained_input_parameter != right.retained_input_parameter
        || !borrow.incoming_references.is_valid(left.incoming_reference)
        || left.incoming_reference != right.incoming_reference
        || !left.source_owner_symbol.is_valid()
        || left.source_owner_symbol != right.source_owner_symbol
        || !left.root.is_valid()
        || left.root != right.root
        || left.kind != right.kind
        || !borrow
            .access_segments
            .span(left.incoming_owner_path)
            .is_some_and(|path| {
                Some(path) == borrow.access_segments.span(right.incoming_owner_path)
            })
        || !borrow
            .access_segments
            .span(left.segments)
            .is_some_and(|path| Some(path) == borrow.access_segments.span(right.segments))
        || !borrow
            .loan_suppliers
            .span(left.supplying_loans)
            .is_some_and(|suppliers| {
                Some(suppliers) == borrow.loan_suppliers.span(right.supplying_loans)
            })
    {
        return false;
    }
    let completion = borrow.calls.get(left.established_by_call);
    if completion.authored_expression != left.formation_expression
        || !completion
            .completed_reference_loans
            .handles()
            .any(|loan| loan == left_handle)
        || !completion
            .completed_reference_loans
            .handles()
            .any(|loan| loan == right_handle)
    {
        return false;
    }
    let mut calls = state.calls.handles().filter_map(|handle| {
        let call = facts.flow.control.calls.get(handle);
        (call.statement_index == completion.statement_index
            && call.call_ordinal == completion.call_ordinal
            && call.target_symbol == completion.target_symbol
            && call.authored_expression == left.formation_expression)
            .then_some(call)
    });
    let Some(call) = calls.next() else {
        return false;
    };
    if calls.next().is_some() {
        return false;
    }
    let Some(argument) = facts
        .flow
        .control
        .call_arguments
        .span(call.authored_arguments)
        .and_then(|arguments| arguments.get(parameter))
    else {
        return false;
    };
    let Some(input) = call
        .operands
        .handles()
        .nth(parameter + usize::from(completion.has_receiver))
    else {
        return false;
    };
    let input = facts.flow.control.operands.get(input);
    // The operand, rather than its backing address, identifies the saved
    // value and storage version. Two formal inputs remain distinct even if
    // their evaluated pointers happen to select the same storage.
    input.expression == *argument
        && input.state_symbol == state.state_symbol
        && input.statement_index == completion.statement_index
        && facts
            .flow
            .control
            .storage_snapshots
            .is_valid(input.storage_snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{BorrowCallFact, FlowCallFact, FlowOperandFact};
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    fn completed_inputs() -> (CheckFacts, FlowStateFact, [Handle<BorrowLoanFact>; 2]) {
        let mut facts = CheckFacts::default();
        let state_symbol = SymbolHandle::from_parts(1, 1);
        let input_owner = SymbolHandle::from_parts(2, 1);
        let target = SymbolHandle::from_parts(3, 1);
        let expression = ExpressionHandle::from_parts(1, 1);
        let argument = ExpressionHandle::from_parts(2, 1);
        let storage_snapshot = facts
            .flow
            .control
            .storage_snapshots
            .insert(Default::default());
        let operands = facts.flow.control.operands.insert_many([FlowOperandFact {
            expression: argument,
            state_symbol,
            storage_snapshot,
            ..Default::default()
        }]);
        let arguments = facts.flow.control.call_arguments.insert_many([argument]);
        let calls = facts.flow.control.calls.insert_many([FlowCallFact {
            target_symbol: target,
            authored_expression: expression,
            authored_arguments: arguments,
            operands,
            ..Default::default()
        }]);
        let completion = facts.borrow.calls.insert(BorrowCallFact {
            target_symbol: target,
            authored_expression: expression,
            ..Default::default()
        });
        let incoming_reference = facts.borrow.incoming_references.insert(Default::default());
        let row = BorrowLoanFact {
            lineage: BorrowLoanLineage::LifetimeDependency,
            owner_symbol: state_symbol,
            storage_owner: PlaceRoot::Expression(expression),
            formation_expression: expression,
            established_by_call: completion,
            retained_input_parameter: Some(0),
            incoming_reference,
            source_owner_symbol: input_owner,
            root: PlaceRoot::Symbol(input_owner),
            kind: checked_trees::BorrowAccessKind::Mutable,
            ..Default::default()
        };
        let loans = facts.borrow.loans.insert_many([row.clone(), row]);
        facts
            .borrow
            .calls
            .get_mut(completion)
            .completed_reference_loans = loans;
        let state = FlowStateFact {
            state_symbol,
            calls,
            ..Default::default()
        };
        (facts, state, [loans.handle_at(0), loans.handle_at(1)])
    }

    #[test]
    fn repeated_completed_input_restrictions_share_one_formation() {
        let (mut facts, state, [left, right]) = completed_inputs();
        let output = facts.borrow.owner_segments.insert_many([
            checked_trees::BorrowLoanOwnerSegment::Field(SymbolHandle::from_parts(4, 1)),
        ]);
        facts.borrow.loans.get_mut(right).owner_path = output;
        assert!(same_completed_input_debt(&facts, &state, left, right));
        assert!(same_completed_input_debt(&facts, &state, right, left));
    }

    #[test]
    fn equal_backing_does_not_merge_distinct_formal_inputs() {
        let (mut facts, state, [left, right]) = completed_inputs();
        facts.borrow.loans.get_mut(right).retained_input_parameter = Some(1);
        assert!(!same_completed_input_debt(&facts, &state, left, right));
    }

    #[test]
    fn a_later_completion_cannot_reuse_an_earlier_input_capture() {
        let (mut facts, state, [left, right]) = completed_inputs();
        let original = facts.borrow.loans.get(left).established_by_call;
        let later = facts
            .borrow
            .calls
            .insert(facts.borrow.calls.get(original).clone());
        facts.borrow.loans.get_mut(right).established_by_call = later;
        assert!(!same_completed_input_debt(&facts, &state, left, right));
    }

    #[test]
    fn an_unavailable_storage_version_cannot_identify_a_completed_input() {
        let (mut facts, state, [left, right]) = completed_inputs();
        let operand = facts
            .flow
            .control
            .calls
            .get(state.calls.handle_at(0))
            .operands
            .handle_at(0);
        facts
            .flow
            .control
            .operands
            .get_mut(operand)
            .storage_snapshot = Handle::invalid();
        assert!(!same_completed_input_debt(&facts, &state, left, right));
    }

    #[test]
    fn descriptor_permissions_still_require_their_own_compatibility() {
        let (mut facts, state, [left, right]) = completed_inputs();
        facts.borrow.loans.get_mut(right).lineage = BorrowLoanLineage::DirectRoot;
        assert!(!same_completed_input_debt(&facts, &state, left, right));
    }
}
