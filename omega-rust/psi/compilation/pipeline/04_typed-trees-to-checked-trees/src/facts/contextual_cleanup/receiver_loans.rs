//! A selected residual remains an intact nominal receiver until its admitted
//! death. Lexical observations of a carried descriptor cannot end that debt.
use checked_trees::{BorrowFacts, ContextualCleanupFacts, FlowOwnershipFacts};
use language_semantics::PermissionEventSource;

pub(crate) fn retain_selected_receiver_loans(
    program: &dyn validation::CheckingInput,
    borrow: &mut BorrowFacts,
    ownership: &FlowOwnershipFacts,
    cleanup: &ContextualCleanupFacts,
) -> bool {
    let mut changes: Vec<(arena::Handle<checked_trees::BorrowLoanFact>, usize)> = Vec::new();
    for (_, disposition) in cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| row.selection.is_valid())
    {
        let receipt = ownership.owned_selections.get(disposition.selection);
        let Some(state) = program.state_by_symbol(disposition.state) else {
            continue;
        };
        let death = match disposition.source {
            PermissionEventSource::StateExit => disposition.exit.statement_index,
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => continue,
        };
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        // `last_use` names an executed statement. The state-exit coordinate
        // itself has a position, but retaining through that boundary would
        // leave no following weakening for the exact resource lifecycle.
        let death = if order
            .position(death)
            .is_some_and(|position| position < order.indices().len())
        {
            death
        } else {
            let Some(last) = order.indices().last().copied() else {
                continue;
            };
            last
        };
        for (handle, loan) in borrow.loans.iter().filter(|(_, loan)| {
            Some(loan.owner_symbol) == disposition.root.as_symbol()
                && order.precedes(loan.statement_index, receipt.statement_ordinal as usize)
        }) {
            let Some(reference) =
                validation::reference_result_custody::storage_owner_type_reference(
                    program,
                    state,
                    loan.owner_symbol,
                    receipt.statement_ordinal,
                )
            else {
                continue;
            };
            let path = crate::borrow::owner_place_segments(borrow.loan_owner_path(loan));
            if validation::reference_result_custody::path_has_nominal_receiver(
                program, reference, &path,
            ) && order.precedes(loan.last_use_statement_index, death)
            {
                if let Some((_, previous)) = changes
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == handle)
                {
                    if order.precedes(*previous, death) {
                        *previous = death;
                    }
                } else {
                    changes.push((handle, death));
                }
            }
        }
    }
    let changed = !changes.is_empty();
    for (handle, death) in changes {
        borrow.loans.get_mut(handle).last_use_statement_index = death;
    }
    changed
}
