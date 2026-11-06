//! One completed call may record the same input restriction on several outputs.
//! Authenticate its saved input once, then compare the output holders without
//! searching the call or completed-loan rosters again for every pair.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CheckFacts, FlowCallFact, FlowStateFact,
};

use super::same_carrier_generation;

pub(in crate::checks::borrows) struct InvocationArguments<'program> {
    rows: Vec<(
        typed_trees::expression::ExpressionHandle,
        Option<&'program [typed_trees::expression::ExpressionHandle]>,
    )>,
}

impl<'program> InvocationArguments<'program> {
    pub(in crate::checks::borrows) fn get(
        &self,
        expression: &typed_trees::expression::ExpressionHandle,
    ) -> Option<&Option<&'program [typed_trees::expression::ExpressionHandle]>> {
        let position = self
            .rows
            .binary_search_by_key(expression, |(expression, _)| *expression)
            .ok()?;
        Some(&self.rows[position].1)
    }
}

/// Typed traversal and borrow collection do not count the same operations.
/// Keep each expression's identity, rather than sharing their ordinal numbers.
pub(in crate::checks::borrows) fn statement_input_arguments<'program>(
    program: &'program dyn validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
) -> InvocationArguments<'program> {
    input_argument_index(
        validation::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .filter_map(|site| {
                let validation::CallSite::Expression { expression, .. } = site else {
                    // Statement calls and named transfers have no expression
                    // result identity. Their output custody is checked separately.
                    return None;
                };
                Some((
                    expression,
                    validation::call_site_argument_expressions(program, &site),
                ))
            }),
    )
}

fn input_argument_index<'program>(
    inputs: impl IntoIterator<
        Item = (
            typed_trees::expression::ExpressionHandle,
            &'program [typed_trees::expression::ExpressionHandle],
        ),
    >,
) -> InvocationArguments<'program> {
    let mut inputs = inputs
        .into_iter()
        .filter(|(expression, _)| expression.is_valid())
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|(expression, _)| *expression);
    let mut rows: Vec<(_, Option<&[_]>)> = Vec::with_capacity(inputs.len());
    for (expression, arguments) in inputs {
        if let Some((prior, roster)) = rows.last_mut()
            && *prior == expression
        {
            *roster = None;
        } else {
            rows.push((expression, Some(arguments)));
        }
    }
    InvocationArguments { rows }
}

/// Construction authenticates the exact call input and its saved storage
/// version. Private fields ensure comparisons use only the authenticated input.
pub(in crate::checks::borrows) struct CompletedInputDebt<'facts> {
    loan: &'facts BorrowLoanFact,
    completed_loans: HandleSpan<BorrowLoanFact>,
}

/// The statement checker already selected the unique performed call. Reuse it
/// while preparing this loan, before comparing any active loan pairs.
pub(in crate::checks::borrows) fn prepare_completed_input_debt<'facts>(
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    loan_handle: Handle<BorrowLoanFact>,
    call: &FlowCallFact,
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Option<CompletedInputDebt<'facts>> {
    let borrow = &facts.borrow;
    if !borrow.loans.is_valid(loan_handle) {
        return None;
    }
    let loan = borrow.loans.get(loan_handle);
    let parameter = loan.retained_input_parameter?;
    if loan.lineage != BorrowLoanLineage::LifetimeDependency
        || !borrow.calls.is_valid(loan.established_by_call)
        || !loan.formation_expression.is_valid()
    {
        return None;
    }
    let completion = borrow.calls.get(loan.established_by_call);
    if completion.authored_expression != loan.formation_expression
        || !completion.completed_reference_loans.contains(loan_handle)
        || call.statement_index != completion.statement_index
        || call.statement_index != loan.statement_index
        || call.call_ordinal != completion.call_ordinal
        || call.target_symbol != completion.target_symbol
        || call.authored_expression != loan.formation_expression
    {
        return None;
    }
    // Source-custody publication fills call.authored_arguments after checking.
    // The checking caller supplies this exact expression's typed invocation inputs.
    let argument = arguments.get(parameter)?;
    let position = parameter.checked_add(usize::from(completion.has_receiver))?;
    let input_handle = call.operands.handles().nth(position)?;
    if !facts.flow.control.operands.is_valid(input_handle) {
        return None;
    }
    let input = facts.flow.control.operands.get(input_handle);
    // completed_storage::versions defines canonical zero as incoming storage.
    // A stale or malformed nonzero handle is not that entry version. This
    // check follows capture validation above; missing operands supply nothing.
    let storage_version_exists = input.storage_snapshot == Handle::invalid()
        || facts
            .flow
            .control
            .storage_snapshots
            .is_valid(input.storage_snapshot);
    if input.expression != *argument
        || input.state_symbol != state.state_symbol
        || input.statement_index != completion.statement_index
        || !storage_version_exists
    {
        return None;
    }
    Some(CompletedInputDebt {
        loan,
        completed_loans: completion.completed_reference_loans,
    })
}

/// Recording one captured input restriction on several outputs does not borrow
/// it twice. Both rows remain live for their separate output-leaf lifetimes.
/// A different formal input, completion or descriptor permission cannot match.
pub(in crate::checks::borrows) fn same_completed_input_debt(
    borrow: &BorrowFacts,
    prepared: Option<&CompletedInputDebt<'_>>,
    right_handle: Handle<BorrowLoanFact>,
) -> bool {
    let Some(prepared) = prepared else {
        return false;
    };
    if !borrow.loans.is_valid(right_handle) || !prepared.completed_loans.contains(right_handle) {
        return false;
    }
    let left = prepared.loan;
    let right = borrow.loans.get(right_handle);
    left.lineage == BorrowLoanLineage::LifetimeDependency
        && right.lineage == BorrowLoanLineage::LifetimeDependency
        && same_carrier_generation(left, right)
        && left.formation_expression == right.formation_expression
        && left.retained_input_parameter == right.retained_input_parameter
        && borrow.incoming_references.is_valid(left.incoming_reference)
        && left.incoming_reference == right.incoming_reference
        && left.source_owner_symbol.is_valid()
        && left.source_owner_symbol == right.source_owner_symbol
        && left.root.is_valid()
        && left.root == right.root
        && left.kind == right.kind
        && borrow
            .access_segments
            .span(left.incoming_owner_path)
            .is_some_and(|path| {
                Some(path) == borrow.access_segments.span(right.incoming_owner_path)
            })
        && borrow
            .access_segments
            .span(left.segments)
            .is_some_and(|path| Some(path) == borrow.access_segments.span(right.segments))
        && borrow
            .loan_suppliers
            .span(left.supplying_loans)
            .is_some_and(|suppliers| {
                Some(suppliers) == borrow.loan_suppliers.span(right.supplying_loans)
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{BorrowCallFact, FlowCallFact, FlowOperandFact};
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    #[test]
    fn a_preceding_probe_does_not_shift_an_invocations_arguments() {
        let probe = ExpressionHandle::from_parts(1, 1);
        let call = ExpressionHandle::from_parts(2, 1);
        let probe_arguments = [ExpressionHandle::from_parts(3, 1)];
        let arguments = [ExpressionHandle::from_parts(4, 1)];
        let rows = input_argument_index([
            (probe, probe_arguments.as_slice()),
            (call, arguments.as_slice()),
        ]);
        assert_eq!(
            rows.get(&call).copied().flatten(),
            Some(arguments.as_slice())
        );
        assert_eq!(
            rows.get(&probe).copied().flatten(),
            Some(probe_arguments.as_slice())
        );
    }

    #[test]
    fn repeated_invocation_identity_cannot_select_an_argument_roster() {
        let call = ExpressionHandle::from_parts(1, 1);
        let arguments = [ExpressionHandle::from_parts(2, 1)];
        let rows =
            input_argument_index([(call, arguments.as_slice()), (call, arguments.as_slice())]);
        assert_eq!(rows.get(&call), Some(&None));
    }

    fn completed_inputs() -> (CheckFacts, FlowStateFact, [Handle<BorrowLoanFact>; 2]) {
        let mut facts = CheckFacts::default();
        let state_symbol = SymbolHandle::from_parts(1, 1);
        let input_owner = SymbolHandle::from_parts(2, 1);
        let target = SymbolHandle::from_parts(3, 1);
        let expression = ExpressionHandle::from_parts(1, 1);
        let argument = ExpressionHandle::from_parts(2, 1);
        let other_argument = ExpressionHandle::from_parts(3, 1);
        // The storage graph starts at the canonical incoming-storage version.
        let storage_snapshot = Handle::invalid();
        let operands = facts
            .flow
            .control
            .operands
            .insert_many(
                [argument, other_argument].map(|expression| FlowOperandFact {
                    expression,
                    state_symbol,
                    storage_snapshot,
                    ..Default::default()
                }),
            );
        let calls = facts.flow.control.calls.insert_many([FlowCallFact {
            target_symbol: target,
            authored_expression: expression,
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

    fn same_input(
        facts: &CheckFacts,
        state: &FlowStateFact,
        left: Handle<BorrowLoanFact>,
        right: Handle<BorrowLoanFact>,
    ) -> bool {
        let call = facts.flow.control.calls.get(state.calls.handle_at(0));
        let arguments = [
            ExpressionHandle::from_parts(2, 1),
            ExpressionHandle::from_parts(3, 1),
        ];
        assert!(call.authored_arguments.is_empty());
        let prepared = prepare_completed_input_debt(facts, state, left, call, &arguments);
        same_completed_input_debt(&facts.borrow, prepared.as_ref(), right)
    }

    #[test]
    fn repeated_completed_input_restrictions_share_one_formation() {
        let (mut facts, state, [left, right]) = completed_inputs();
        let output = facts.borrow.owner_segments.insert_many([
            checked_trees::BorrowLoanOwnerSegment::Field(SymbolHandle::from_parts(4, 1)),
        ]);
        facts.borrow.loans.get_mut(right).owner_path = output;
        assert!(same_input(&facts, &state, left, right));
        assert!(same_input(&facts, &state, right, left));
    }

    #[test]
    fn equal_backing_does_not_merge_distinct_formal_inputs() {
        let (mut facts, state, [left, right]) = completed_inputs();
        facts.borrow.loans.get_mut(right).retained_input_parameter = Some(1);
        assert!(!same_input(&facts, &state, left, right));
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
        assert!(!same_input(&facts, &state, left, right));
    }

    #[test]
    fn stale_or_malformed_versions_cannot_identify_a_completed_input() {
        let (mut facts, state, [left, right]) = completed_inputs();
        let operand = facts
            .flow
            .control
            .calls
            .get(state.calls.handle_at(0))
            .operands
            .handle_at(0);
        for version in [Handle::from_parts(99, 2), Handle::from_parts(99, 0)] {
            facts
                .flow
                .control
                .operands
                .get_mut(operand)
                .storage_snapshot = version;
            assert!(!same_input(&facts, &state, left, right));
        }
    }

    #[test]
    fn an_absent_capture_cannot_become_entry_storage() {
        let (mut facts, state, [left, right]) = completed_inputs();
        facts
            .flow
            .control
            .calls
            .get_mut(state.calls.handle_at(0))
            .operands = Default::default();
        assert!(!same_input(&facts, &state, left, right));
    }

    #[test]
    fn descriptor_permissions_still_require_their_own_compatibility() {
        let (mut facts, state, [left, right]) = completed_inputs();
        facts.borrow.loans.get_mut(right).lineage = BorrowLoanLineage::DirectRoot;
        assert!(!same_input(&facts, &state, left, right));
    }
}
