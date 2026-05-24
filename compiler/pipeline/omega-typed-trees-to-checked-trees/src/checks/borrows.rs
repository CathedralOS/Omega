use omega_checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use omega_checked_trees::expression::ExpressionHandle;
use omega_core::diagnostics::Diagnostic;

use crate::labels::{call_target_label, symbol_name};
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
    let writable_roots: Vec<_> = facts
        .flow
        .borrow_writable_root_constraints(state_flow.entry_constraints)
        .map(|root| symbol_name(program, facts.borrow.writable_roots.get(root).symbol))
        .collect();
    let accesses: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .collect();

    for (index, access) in accesses.iter().enumerate() {
        if access.kind != BorrowAccessKind::Mutable {
            continue;
        }

        for other_access in accesses.iter().skip(index + 1) {
            if access.root_symbol != other_access.root_symbol {
                continue;
            }

            match other_access.kind {
                BorrowAccessKind::Mutable => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    symbol_name(program, access.root_symbol),
                ))),
                BorrowAccessKind::Read => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    symbol_name(program, access.root_symbol),
                ))),
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
