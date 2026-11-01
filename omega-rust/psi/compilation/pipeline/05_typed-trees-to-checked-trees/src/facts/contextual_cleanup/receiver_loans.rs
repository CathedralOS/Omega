//! Every disposed residual remains an intact nominal receiver until its
//! admitted death, whether its owner came from an ordinary establishment or a
//! conditional selection. Lexical observations cannot end its carried loans
//! before the hook's implicit use. The ownership producer chooses the residual;
//! this fixed-point extension preserves its receiver through that boundary.
use checked_trees::{BorrowFacts, ContextualCleanupFacts, FlowOwnershipFacts};
use language_semantics::PermissionEventSource;

pub(crate) fn retain_selected_receiver_loans(
    program: &dyn validation::CheckingInput,
    borrow: &mut BorrowFacts,
    ownership: &FlowOwnershipFacts,
    cleanup: &ContextualCleanupFacts,
) -> bool {
    let mut changes: Vec<(arena::Handle<checked_trees::BorrowLoanFact>, usize)> = Vec::new();
    for (_, disposition) in cleanup.dispositions.iter() {
        if disposition.zero_image {
            continue;
        }
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
        let receiver_coordinate = if disposition.selection.is_valid() {
            ownership
                .owned_selections
                .get(disposition.selection)
                .statement_ordinal as usize
        } else {
            death
        };
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
                && (loan.statement_index == receiver_coordinate
                    || order.precedes(loan.statement_index, receiver_coordinate))
        }) {
            let Some(reference) =
                validation::reference_result_custody::storage_owner_type_reference(
                    program,
                    state,
                    loan.owner_symbol,
                    receiver_coordinate as u32,
                )
            else {
                continue;
            };
            let path = crate::borrow::owner_place_segments(borrow.loan_owner_path(loan));
            if crate::borrow::place_path_covers_owner(
                program,
                ownership.segments.span_or_empty(disposition.path),
                borrow.loan_owner_path(loan),
            ) && ![disposition.exclusions, disposition.inactive_cases]
                .into_iter()
                .flat_map(|regions| cleanup.exclusions.span_or_empty(regions))
                .any(|excluded| {
                    excluded.root == disposition.root
                        && crate::borrow::owner_path_overlaps_place_segments(
                            program,
                            borrow.loan_owner_path(loan),
                            cleanup.segments.span_or_empty(excluded.segments),
                        )
                })
                && validation::reference_result_custody::path_has_nominal_receiver(
                    program, reference, &path,
                )
                && order.precedes(loan.last_use_statement_index, death)
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
