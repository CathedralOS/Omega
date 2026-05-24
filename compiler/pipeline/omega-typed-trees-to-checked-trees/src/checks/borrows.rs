use omega_checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use omega_checked_trees::expression::ExpressionHandle;
use omega_core::diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, call_target_label, symbol_name};
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};

pub(crate) fn check_flow_call_borrows(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.states.iter() {
        let Some(borrow_state) = facts.borrow.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == state_flow.machine_symbol
                && state.state_symbol == state_flow.state_symbol)
                .then_some(state)
        }) else {
            continue;
        };

        for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
            check_call_borrows(program, facts, state_flow, borrow_call, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_call_borrows(
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
            if !facts.borrow.accesses_overlap(access, other_access) {
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
            if facts.borrow.access_overlaps_loan(access, loan) {
                let detail =
                    active_loan_detail(state_flow, facts, *loan_handle, borrow_call.statement_index);
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
    facts.flow
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .find(|call| {
            call.statement_index == borrow_call.statement_index
                && call.call_ordinal == borrow_call.call_ordinal
                && call.target_symbol == borrow_call.target_symbol
                && call.receiver_symbol == borrow_call.receiver_symbol
        })
        .map(|call| call.entry_constraints)
        .unwrap_or(state_flow.entry_constraints)
}

fn mutable_argument_root_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        omega_checked_trees::expression::ExpressionNode::Indexed(indexed) => {
            mutable_argument_root_name(program, indexed.collection)
        }
        omega_checked_trees::expression::ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                omega_checked_trees::expression::ExpressionNode::Name(path) => {
                    let members = program.expression_table.name_path_members(path.members);
                    if members.first().is_some_and(|member_name| member_name.as_str() == "self") {
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

fn active_loan_detail(
    state_flow: &FlowStateFact,
    facts: &CheckFacts,
    loan: omega_core::arena::Handle<omega_checked_trees::BorrowLoanFact>,
    statement_index: usize,
) -> Option<String> {
    facts.flow
        .borrow_weakenings
        .span_or_empty(state_flow.borrow_weakenings)
        .iter()
        .find(|weakening| weakening.loan == loan)
        .and_then(|weakening| {
            let loan = facts.borrow.loans.get(loan);
            match (weakening.reason, weakening.source) {
            (
                omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                omega_checked_trees::FlowInvalidationSource::Statement {
                    statement_index: weakening_statement,
                },
            ) if weakening_statement > statement_index => Some(format!(
                "its last use is at statement {}",
                loan.last_use_statement_index
            )),
            (
                omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                omega_checked_trees::FlowInvalidationSource::Statement { .. },
            ) if loan.last_use_statement_index > statement_index => Some(format!(
                "its last use is at statement {} and it is released at state exit",
                loan.last_use_statement_index
            )),
            (
                omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                omega_checked_trees::FlowInvalidationSource::Statement { .. },
            )
            | (
                omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                omega_checked_trees::FlowInvalidationSource::Statement { .. },
            )
            | (
                omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired,
                omega_checked_trees::FlowInvalidationSource::Call { .. },
            )
            | (
                omega_checked_trees::FlowBorrowWeakeningReason::StateExit,
                omega_checked_trees::FlowInvalidationSource::Call { .. },
            ) => None,
        }})
}
