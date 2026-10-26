//! A completed storage observation can release its unused local descriptor on
//! the selected edge. Its exact checked reference-release boundary owns that death;
//! later guards on the missed path do not keep it live on this taken path.
//! Lowering must place that same checked release after the observation and
//! before the target graph, independently checking the completed use inventory.
//! Carried loans, derived holders and unknown origins retain ordinary lifetime
//! checks. This supplies no new loan or restoration authority.

use arena::Handle;
use checked_trees::{BorrowCallFact, BorrowLoanFact, CheckFacts, FlowStateFact};
use typed_trees::statement::TransitionTargetHandle;

pub(super) fn before_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    call: &BorrowCallFact,
    loan: Handle<BorrowLoanFact>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, flow.state_symbol) else {
        return false;
    };
    let checked_trees::FlowPermissionEvaluation::TransitionTarget(target) =
        crate::flow::call_permission_evaluation(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )
    else {
        return false;
    };
    before_target(program, facts, flow, call.statement_index, target, loan)
}

pub(super) fn before_target(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    statement_index: usize,
    target: TransitionTargetHandle,
    loan_handle: Handle<BorrowLoanFact>,
) -> bool {
    let Some(state) = program.state_by_symbol(flow.state_symbol) else {
        return false;
    };
    let Some(transition) = program
        .statement_table
        .statement_at(state, statement_index)
        .and_then(|statement| statement.as_transition())
    else {
        return false;
    };
    if !target.is_valid() || (target != transition.target && target != transition.continuation) {
        return false;
    }
    let loan = facts.borrow.loans.get(loan_handle);
    let Some((declaration_index, local)) =
        program.state_local_declaration(state, loan.owner_symbol)
    else {
        return false;
    };
    if declaration_index >= statement_index
        || local.is_mutable
        || loan.storage_owner != facts::PlaceRoot::Symbol(local.symbol)
        || !loan.owner_path.is_empty()
        || validation::reference_shell(program, local.type_reference).is_none()
        || validation::transition_target_uses_symbol(
            program,
            program.statement_table.transition_target(target),
            loan.owner_symbol,
        )
    {
        return false;
    }
    if !observed_local_read(program, facts, flow, statement_index, loan_handle) {
        return false;
    }
    // A descendant may use only its own descriptor after forming through this
    // one. Keep the existing lifetime whenever any retained child depends on
    // it, including stored copies and alternatives in a completed call row.
    let Some(borrow_state) = super::matching_borrow_state(facts, flow) else {
        return false;
    };
    let Some(mut state_loans) = facts.borrow.loan_group(borrow_state.loans) else {
        return false;
    };
    if state_loans.any(|(handle, child)| {
        let Some(suppliers) = facts.borrow.loan_suppliers.span(child.supplying_loans) else {
            return true;
        };
        handle != loan_handle
            && (child.source_owner_symbol == loan.owner_symbol
                || child.root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                || matches!(child.lineage, checked_trees::BorrowLoanLineage::Reborrow { parent_loan }
                    if parent_loan == loan_handle)
                || suppliers.contains(&loan_handle))
    }) {
        return false;
    }
    let Some(weakening) = validation::reference_result_custody::loan_weakening(
        facts,
        flow.machine_symbol,
        flow.state_symbol,
        loan_handle,
    ) else {
        return false;
    };
    let state_exit = state.statement_nodes.count();
    // Unrestricted reference descriptors have no affine owner disposition.
    // Join their unique checked weakening to the same last-use release query
    // that produces deferred ReleaseReference. Lowering authenticates moving
    // that exact release across the completed read from its full use graph.
    if weakening.reason != checked_trees::FlowBorrowWeakeningReason::StateExit
        || weakening.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: state_exit as usize,
            })
        || validation::reference_result_custody::release_statement(
            program,
            facts,
            flow.machine_symbol,
            flow.state_symbol,
            loan_handle,
        ) != Some(state_exit)
    {
        return false;
    }
    true
}

fn observed_local_read(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    statement_index: usize,
    loan: Handle<BorrowLoanFact>,
) -> bool {
    let owner = facts.borrow.loans.get(loan).owner_symbol;
    let Some(statement) = facts.flow.state_statement(flow, statement_index) else {
        return false;
    };
    // Only an actually completed storage read supplies this endpoint. A type, an
    // initializer or a selected target alone cannot stand in for evaluation.
    facts
        .flow
        .control
        .value_observations
        .span_or_empty(statement.value_observations)
        .iter()
        .any(|observation| {
            if observation.evaluation != checked_trees::FlowPermissionEvaluation::TransitionGuard
                || !facts.flow.control.operands.is_valid(observation.receiver)
                || !facts
                    .flow
                    .borrow_loan_constraints(observation.constraints)
                    .any(|active| active == loan)
            {
                return false;
            }
            let receiver = facts.flow.control.operands.get(observation.receiver);
            receiver.state_symbol == flow.state_symbol
                && receiver.statement_index == statement_index
                && validation::canonical_place_from_expression_in_state(
                    program,
                    flow.state_symbol,
                    statement_index,
                    receiver.expression,
                )
                .is_some_and(|place| {
                    place.root == facts::PlaceRoot::Symbol(owner) && place.segments.is_empty()
                })
        })
}
