use omega_checked_trees::{BorrowAccessKind, CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;

use crate::flow::statement_mutated_place;
use crate::labels::symbol_name;
use crate::semantic_calls::find_state_in_machine;

use super::details::{active_loan_detail, canonical_place_label};
use super::overlap::{borrow_loan_overlaps_loan, canonical_place_overlaps_loan};

pub(super) fn check_statement_borrows(
    program: &omega_typed_trees::TypedTrees,
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

    for statement in facts.flow.statements.span_or_empty(state_flow.statements) {
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
}
