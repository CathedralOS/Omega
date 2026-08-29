use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_checked_trees::{BorrowCallFact, CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use crate::labels::symbol_name;
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};

pub(super) fn check_mutable_argument_writability(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: psi_arena::HandleSpan<psi_checked_trees::FlowConstraintRef>,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let writable_roots: Vec<_> = facts
        .flow
        .borrow_writable_root_constraints(entry_constraints)
        .map(|root| symbol_name(program, facts.borrow.writable_roots.get(root).symbol))
        .collect();

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
        let ExpressionNode::Borrow(inner_expression) =
            program.expression_table.expression(*argument)
        else {
            continue;
        };
        if !inner_expression.access.is_exclusive() {
            continue;
        }

        let Some(root_name) = mutable_argument_root_name(program, inner_expression.target) else {
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
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => mutable_argument_root_name(program, indexed.collection),
        ExpressionNode::Range(_) => None,
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            {
                let members = program.expression_table.name_path_members(path.members);
                if members
                    .first()
                    .is_some_and(|member_name| member_name.as_str() == "self")
                {
                    return Some(member.member.as_str().to_owned());
                }
            }
            mutable_argument_root_name(program, member.receiver)
        }
        ExpressionNode::Borrow(inner_expression) => {
            mutable_argument_root_name(program, inner_expression.target)
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .map(|member| member.as_str().to_owned()),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}
