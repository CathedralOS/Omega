use crate::context::*;

use super::tracker::StateLoanTracker;
use usage::{statement_uses_local_name, statement_uses_symbol};

mod usage;

pub(super) fn update_state_loan_last_uses(
    program: &omega_typed_trees::TypedTrees,
    statements: omega_core::arena::HandleSpan<StatementNode>,
    borrow_calls: &[BorrowCallFact],
    argument_accesses: &omega_core::arena::Arena<BorrowArgumentAccessFact>,
    loan_trackers: &[StateLoanTracker],
    loans: &mut omega_core::arena::Arena<omega_checked_trees::BorrowLoanFact>,
) {
    if loan_trackers.is_empty() {
        return;
    }

    for borrow_call in borrow_calls {
        for access in argument_accesses.span_or_empty(borrow_call.accesses) {
            for tracker in loan_trackers {
                if tracker.owner_symbol == access.root_symbol {
                    loans.get_mut(tracker.handle).last_use_statement_index =
                        borrow_call.statement_index;
                }
            }
        }
    }

    for (statement_index, statement) in program
        .statement_table
        .statements(statements)
        .iter()
        .enumerate()
    {
        for tracker in loan_trackers {
            if statement_uses_local_name(program, statement, tracker.owner_name.as_str())
                || statement_uses_symbol(program, statement, tracker.owner_symbol)
            {
                loans.get_mut(tracker.handle).last_use_statement_index = statement_index;
            }
        }
    }
}
