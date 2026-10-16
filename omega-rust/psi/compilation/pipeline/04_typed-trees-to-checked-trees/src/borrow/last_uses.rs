use checked_trees::statement::StatementNode;
use checked_trees::{BorrowArgumentAccessFact, BorrowCallFact};
use symbols::SymbolHandle;

use super::tracker::StateLoanTracker;
pub(crate) use usage::owner_path_overlaps_place_segments;
use usage::{statement_uses_local_name, statement_uses_owner_path, statement_uses_symbol};

mod usage;

/// Whether a lexical place rooted at `symbol` remains live after one source
/// statement. This is the statement-bound first consumer of canonical
/// liveness outside borrow expiry; finer intra-statement ordering can extend
/// the same query without changing carry semantics.
pub(crate) fn place_is_used_after_statement(
    program: &dyn ::validation::CheckingInput,
    statements: arena::HandleSpan<StatementNode>,
    statement_index: usize,
    symbol: SymbolHandle,
    local_name: &str,
) -> bool {
    program
        .statement_table
        .admitted_execution_statements(statements)
        .skip_while(|(index, _)| *index != statement_index)
        .skip(1)
        .any(|(_, statement)| {
            usage::statement_uses_symbol(program, statement, symbol)
                || usage::statement_uses_local_name(program, statement, local_name)
        })
}

pub(super) fn update_state_loan_last_uses(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statements: arena::HandleSpan<StatementNode>,
    borrow_calls: &[BorrowCallFact],
    access_segments: &arena::Arena<facts::PlaceSegment>,
    argument_accesses: &arena::Arena<BorrowArgumentAccessFact>,
    loan_trackers: &[StateLoanTracker],
    loans: &mut arena::Arena<checked_trees::BorrowLoanFact>,
) {
    if loan_trackers.is_empty() {
        return;
    }

    let order = program.statement_table.admitted_execution_order(statements);

    for borrow_call in borrow_calls {
        for access in argument_accesses.span_or_empty(borrow_call.accesses) {
            for tracker in loan_trackers {
                if !loans.is_valid(tracker.handle) {
                    continue;
                }
                if tracker.storage_owner == access.root
                    && owner_path_overlaps_place_segments(
                        program,
                        &tracker.owner_path,
                        access_segments.span_or_empty(access.segments),
                    )
                {
                    extend_last_use(
                        &order,
                        &mut loans.get_mut(tracker.handle).last_use_statement_index,
                        borrow_call.statement_index,
                    );
                }
            }
        }
    }

    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(statements)
    {
        for tracker in loan_trackers {
            if !loans.is_valid(tracker.handle) {
                continue;
            }
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
                extend_last_use(
                    &order,
                    &mut loans.get_mut(tracker.handle).last_use_statement_index,
                    statement_index,
                );
            }
        }
    }

    // Incoming storage remains live in its caller even when this state makes
    // no further lexical observation of it. Installed field loans end only
    // when replacement replay retires their slot or this state exits.
    if let Some(state) = program.state_by_symbol(state_symbol) {
        for tracker in loan_trackers {
            if !loans.is_valid(tracker.handle) {
                continue;
            }
            if !tracker.owner_path.is_empty()
                && program
                    .state_parameter_by_symbol(state, tracker.owner_symbol)
                    .is_some()
            {
                loans.get_mut(tracker.handle).last_use_statement_index =
                    order.indices().last().copied().unwrap_or_default();
            }
        }
    }

    // A live alias observes its captured carrier even when later source uses
    // spell only the alias. Propagate that lifetime through stored leaves;
    // nested aliases can extend one another, so iterate to the finite maximum
    // of the already observed statement coordinates.
    loop {
        let mut changed = false;
        for carrier in loan_trackers {
            if !loans.is_valid(carrier.handle) || carrier.owner_path.is_empty() {
                continue;
            }
            for alias in loan_trackers {
                if !loans.is_valid(alias.handle)
                    || alias.place.root != carrier.storage_owner
                    || !owner_path_overlaps_place_segments(
                        program,
                        &carrier.owner_path,
                        &alias.place.segments,
                    )
                {
                    continue;
                }
                let alias_last_use = loans.get(alias.handle).last_use_statement_index;
                let carried = loans.get_mut(carrier.handle);
                changed |= extend_last_use(
                    &order,
                    &mut carried.last_use_statement_index,
                    alias_last_use,
                );
            }
        }
        if !changed {
            break;
        }
    }
}

fn extend_last_use(
    order: &typed_trees::statement::StatementExecutionOrder,
    previous: &mut usize,
    next: usize,
) -> bool {
    let Some(next_position) = order.position(next) else {
        return false;
    };
    if order
        .position(*previous)
        .is_some_and(|position| position >= next_position)
    {
        return false;
    }
    *previous = next;
    true
}
