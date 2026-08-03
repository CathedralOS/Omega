use psi_checked_trees::{BorrowAccessKind, CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use crate::flow::{StateMutationSummaryCache, call_mutated_places, statement_mutated_place};
use crate::labels::symbol_name;
use crate::semantic_calls::find_state_in_machine;

use super::details::{active_loan_detail, canonical_place_label};
use super::overlap::{borrow_loan_overlaps_loan, canonical_place_overlaps_loan};

pub(super) fn check_statement_borrows(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(state) =
        find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };
    let Some(borrow_state) = facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    }) else {
        return;
    };

    for statement in facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
    {
        let Some(statement_node) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement.statement_index)
        else {
            continue;
        };

        for loan in facts
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| loan.statement_index == statement.statement_index)
        {
            for active_loan_handle in facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
            {
                let active_loan = facts.borrow.loans.get(active_loan_handle);
                if loan.kind == BorrowAccessKind::Read && active_loan.kind == BorrowAccessKind::Read
                {
                    continue;
                }
                if loan.source_owner_symbol == active_loan.owner_symbol
                    && active_loan.kind == BorrowAccessKind::Mutable
                {
                    continue;
                }

                if !borrow_loan_overlaps_loan(program, facts, loan, active_loan) {
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "statement {} creates local borrow `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    symbol_name(program, loan.owner_symbol),
                    symbol_name(program, active_loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        active_loan_handle,
                        statement.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", active_loan.statement_index)),
                )));
            }
        }

        let Some(mutated_place) = statement_mutated_place(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            statement.statement_index,
            statement_node,
        ) else {
            continue;
        };

        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            let loan = facts.borrow.loans.get(loan_handle);
            if canonical_place_overlaps_loan(program, &mutated_place, loan, &facts.borrow) {
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    canonical_place_label(program, &mutated_place),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(state_flow, facts, loan_handle, statement.statement_index)
                        .unwrap_or_else(|| format!(
                            "borrowed at statement {}",
                            loan.statement_index
                        )),
                )));
            }
        }
    }

    check_call_mutation_borrows(program, facts, state_flow, borrow_state, diagnostics);
}

/// Vec-views borrow rule (and, generally, owner-mutation-through-a-call vs a
/// live borrowed view).
///
/// A mutating call through an owner -- e.g. `Vec::push`/`Vec::index_mut` or any
/// `&mut self` boundary/state call that reallocates or writes the owner -- may
/// invalidate a borrowed slice/string view (`&[T]`/`&mut [T]`/`&string`) taken
/// from that owner. The owner-write rule already rejects this for *assignment*
/// statements; this extends the same conflict to *call* statements, whose
/// mutated places are computed by [`call_mutated_places`] (the receiver place
/// plus any mutable-argument places). A mutated place overlapping a loan that is
/// still live at the call point is rejected.
///
/// This reuses the existing loan-overlap engine, so it inherits the disjoint
/// subslice/element precision and stays conservative when a window is unknown.
fn check_call_mutation_borrows(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_state: &psi_checked_trees::StateBorrowFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut summary_cache = StateMutationSummaryCache::default();

    for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
        let mutated_places = call_mutated_places(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            borrow_call,
            &mut summary_cache,
        );
        if mutated_places.is_empty() {
            continue;
        }

        // The loans live *at* the call are the entry constraints of the flow
        // statement the call belongs to.
        let Some(statement) = facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
            .iter()
            .find(|statement| statement.statement_index == borrow_call.statement_index)
        else {
            continue;
        };

        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            let loan = facts.borrow.loans.get(loan_handle);
            for mutated_place in &mutated_places {
                if !canonical_place_overlaps_loan(program, mutated_place, loan, &facts.borrow) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                    borrow_call.statement_index,
                    canonical_place_label(program, mutated_place),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        loan_handle,
                        borrow_call.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
                )));
                // One diagnostic per (call, loan) is enough.
                break;
            }
        }
    }
}
