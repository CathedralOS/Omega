use crate::context::*;

use super::tracker::StateLoanTracker;
use usage::{
    owner_path_overlaps_place_segments, statement_uses_local_name, statement_uses_owner_path,
    statement_uses_symbol,
};

mod usage;

/// Whether a lexical place rooted at `symbol` remains live after one source
/// statement. This is the statement-bound first consumer of canonical
/// liveness outside borrow expiry; finer intra-statement ordering can extend
/// the same query without changing carry semantics.
pub(crate) fn place_is_used_after_statement(
    program: &psi_typed_trees::TypedTrees,
    statements: psi_arena::HandleSpan<StatementNode>,
    statement_index: usize,
    symbol: SymbolHandle,
    local_name: &str,
) -> bool {
    program
        .statement_table
        .statements(statements)
        .iter()
        .skip(statement_index.saturating_add(1))
        .any(|statement| {
            usage::statement_uses_symbol(program, statement, symbol)
                || usage::statement_uses_local_name(program, statement, local_name)
        })
}

/// Symbol-only form used for persistent field paths. Unlike lexical locals,
/// field identity must never fall back to a spelling: two unrelated records
/// may both have a field named `value`, while their symbols remain distinct.
pub(crate) fn place_symbol_is_used_after_statement(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statements: psi_arena::HandleSpan<StatementNode>,
    statement_index: usize,
    symbol: SymbolHandle,
) -> bool {
    program
        .statement_table
        .statements(statements)
        .iter()
        .enumerate()
        .skip(statement_index.saturating_add(1))
        .any(|(later_index, statement)| {
            usage::statement_uses_place_symbol(
                program,
                state_symbol,
                later_index,
                statement,
                symbol,
            )
        })
}

pub(crate) fn place_symbol_is_used_in_state(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    symbol: SymbolHandle,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .any(|(statement_index, statement)| {
            usage::statement_uses_place_symbol(
                program,
                state.symbol,
                statement_index,
                statement,
                symbol,
            )
        })
}

pub(super) fn update_state_loan_last_uses(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statements: psi_arena::HandleSpan<StatementNode>,
    borrow_calls: &[BorrowCallFact],
    access_segments: &psi_arena::Arena<psi_facts::PlaceSegment>,
    argument_accesses: &psi_arena::Arena<BorrowArgumentAccessFact>,
    loan_trackers: &[StateLoanTracker],
    loans: &mut psi_arena::Arena<psi_checked_trees::BorrowLoanFact>,
) {
    if loan_trackers.is_empty() {
        return;
    }

    for borrow_call in borrow_calls {
        for access in argument_accesses.span_or_empty(borrow_call.accesses) {
            for tracker in loan_trackers {
                if tracker.owner_symbol == access.root_symbol
                    && owner_path_overlaps_place_segments(
                        program,
                        &tracker.owner_path,
                        access_segments.span_or_empty(access.segments),
                    )
                {
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
            let used = if tracker.owner_path.is_empty() {
                statement_uses_local_name(program, statement, tracker.owner_name.as_str())
                    || statement_uses_symbol(program, statement, tracker.owner_symbol)
            } else {
                statement_uses_owner_path(
                    program,
                    state_symbol,
                    statement_index,
                    statement,
                    tracker.owner_symbol,
                    tracker.owner_name.as_str(),
                    &tracker.owner_path,
                )
            };
            if used {
                loans.get_mut(tracker.handle).last_use_statement_index = statement_index;
            }
        }
    }
}
