use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::checks::borrows::details::binding_reference_access;
use crate::checks::borrows::resources::invalid_reborrow_attenuation_diagnostic;
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};

pub(super) fn check_mutable_argument_writability(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

        let root_is_writable = facts
            .flow
            .borrow_writable_root_constraints(entry_constraints)
            .any(|root| {
                let root = facts.borrow.writable_roots.get(root);
                program.symbols.name(root.symbol) == root_name.as_str()
            });
        if !root_is_writable {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument `{root_name}` for state `{target_name}` is not writable in this state"
            )));
            continue;
        }

        // A direct `&mut`/`&write` argument formed on a reference-typed
        // binding is a transient referent reborrow: the binding's own storage
        // is a writable local root, so the writable-root test above cannot
        // see the referent's authority. The binding's declared access is the
        // parent authority and the lattice's access-pair rule decides the
        // formation — the same edge a `let`-bound reborrow of this binding
        // faces. `&write r` on `r: &u8` can never derive write authority from
        // a shared referent, no matter how briefly the borrow lives.
        let child_access = match inner_expression.access {
            language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
            language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
            language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
        };
        if let Some(parent_access) = borrow_target_reference_access(
            program,
            state_flow,
            borrow_call.statement_index,
            inner_expression.target,
        ) && parent_access
            .direct_reborrow_effect(&child_access)
            .is_none()
        {
            diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                &parent_access,
                &child_access,
            ));
        }
    }
}

/// The declared reference access of the parameter or local that an exclusive
/// argument borrow's target place resolves to, when it does. The place's root
/// is what `&mut`/`&write` actually reborrows through, so this answers the
/// parent side of the reborrow access-pair rule for transient arguments.
fn borrow_target_reference_access(
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    statement_index: usize,
    target: ExpressionHandle,
) -> Option<BorrowAccessKind> {
    let place = crate::borrow::accesses::borrow_access_place(
        program,
        state_flow.state_symbol,
        statement_index,
        target,
        state_flow.machine_symbol,
    )?;
    binding_reference_access(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        statement_index,
        place.root_symbol,
    )
}

fn mutable_argument_root_name(
    program: &typed_trees::TypedTrees,
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
        | ExpressionNode::Match(_)
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
