use checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::labels::call_target_label;

mod conflicts;
mod receiver;
mod writability;

use self::conflicts::check_call_access_conflicts;
use self::writability::check_mutable_argument_writability;

pub(super) fn check_call_borrows(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target_name = call_target_label(program, borrow_call.target_symbol);
    let entry_constraints = call_borrow_constraints(borrow_call, state_flow, facts);
    check_call_access_conflicts(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        diagnostics,
    );
    receiver::check_write_only_receiver_conflicts(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        diagnostics,
    );

    check_mutable_argument_writability(
        program,
        facts,
        state_flow,
        borrow_call,
        entry_constraints,
        &target_name,
        diagnostics,
    );
}

fn call_borrow_constraints<'a>(
    borrow_call: &BorrowCallFact,
    state_flow: &'a FlowStateFact,
    facts: &'a CheckFacts,
) -> arena::HandleSpan<checked_trees::FlowConstraintRef> {
    facts.flow.state_call_entry_constraints(
        state_flow,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
        borrow_call.target_symbol,
        borrow_call.receiver_symbol,
    )
}
