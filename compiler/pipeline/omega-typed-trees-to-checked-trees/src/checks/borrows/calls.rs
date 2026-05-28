use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, call_target_label, symbol_name};
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};

use super::details::active_loan_detail;
use super::overlap::{borrow_access_overlaps_loan, borrow_accesses_overlap};

pub(super) fn check_call_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target_name = call_target_label(program, borrow_call.target_symbol);
    let entry_constraints = call_borrow_constraints(borrow_call, state_flow, facts);
    let writable_roots: Vec<_> = facts
        .flow
        .borrow_writable_root_constraints(entry_constraints)
        .map(|root| symbol_name(program, facts.borrow.writable_roots.get(root).symbol))
        .collect();
    let accesses: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .collect();
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .map(|loan| (loan, facts.borrow.loans.get(loan)))
        .collect();

    for (index, access) in accesses.iter().enumerate() {
        if access.kind != BorrowAccessKind::Mutable {
            continue;
        }

        for other_access in accesses.iter().skip(index + 1) {
            if !borrow_accesses_overlap(program, facts, access, other_access) {
                continue;
            }

            match other_access.kind {
                BorrowAccessKind::Mutable => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
                BorrowAccessKind::Read => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
            }
        }

        for (loan_handle, loan) in &active_loans {
            if borrow_access_overlaps_loan(program, facts, access, loan) {
                let detail = active_loan_detail(
                    state_flow,
                    facts,
                    *loan_handle,
                    borrow_call.statement_index,
                );
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` while local borrow `{}` is still active{}",
                    borrow_access_label(program, &facts.borrow, access),
                    symbol_name(program, loan.owner_symbol),
                    detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default(),
                )));
            }
        }
    }

    let Some(call_site) = find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };

    for argument in call_site_argument_expressions(program, &call_site) {
        let omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression) =
            program.expression_table.expression(*argument)
        else {
            continue;
        };

        let Some(root_name) = mutable_argument_root_name(program, *inner_expression) else {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument for state `{target_name}` must be a named place"
            )));
            continue;
        };

        if !writable_roots.contains(&root_name) {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument `{root_name}` for state `{target_name}` is not writable in this state"
            )));
        }
    }
}

fn call_borrow_constraints<'a>(
    borrow_call: &BorrowCallFact,
    state_flow: &'a FlowStateFact,
    facts: &'a CheckFacts,
) -> omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef> {
    facts.flow.state_call_entry_constraints(
        state_flow,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
        borrow_call.target_symbol,
        borrow_call.receiver_symbol,
    )
}

fn mutable_argument_root_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        omega_checked_trees::expression::ExpressionNode::Indexed(indexed) => {
            mutable_argument_root_name(program, indexed.collection)
        }
        omega_checked_trees::expression::ExpressionNode::Range(_) => None,
        omega_checked_trees::expression::ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                omega_checked_trees::expression::ExpressionNode::Name(path) => {
                    let members = program.expression_table.name_path_members(path.members);
                    if members
                        .first()
                        .is_some_and(|member_name| member_name.as_str() == "self")
                    {
                        return Some(member.member.as_str().to_owned());
                    }
                }
                _ => {}
            }
            mutable_argument_root_name(program, member.receiver)
        }
        omega_checked_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            mutable_argument_root_name(program, *inner_expression)
        }
        omega_checked_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|member| member.as_str().to_owned()),
        omega_checked_trees::expression::ExpressionNode::ArrayLiteral(_)
        | omega_checked_trees::expression::ExpressionNode::Binary(_)
        | omega_checked_trees::expression::ExpressionNode::Boolean(_)
        | omega_checked_trees::expression::ExpressionNode::Call(_)
        | omega_checked_trees::expression::ExpressionNode::Cast(_)
        | omega_checked_trees::expression::ExpressionNode::Float(_)
        | omega_checked_trees::expression::ExpressionNode::Integer(_)
        | omega_checked_trees::expression::ExpressionNode::String(_)
        | omega_checked_trees::expression::ExpressionNode::StructLiteral(_) => None,
    }
}
