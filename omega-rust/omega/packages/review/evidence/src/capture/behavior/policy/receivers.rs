//! Receiver identities must survive independently of rendered write-frame paths.

use super::rejected;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn validate_call_receiver_roots(
    source: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    for (_, call) in facts.flow.control.calls.iter() {
        if !call.has_receiver || !call.authored_expression.is_valid() {
            continue;
        }
        let ExpressionNode::Call(expression) =
            source.expression_table.expression(call.authored_expression)
        else {
            continue;
        };
        if !expression.receiver.is_valid() {
            return Err(rejected("checked call lost its source receiver"));
        }
        validate_receiver_root(source, expression.receiver)?;
    }
    Ok(())
}

fn validate_receiver_root(
    source: &TypedTrees,
    mut receiver: ExpressionHandle,
) -> Result<(), Vec<Diagnostic>> {
    // A path cannot visit more nodes than the expression table contains.
    // This also rejects cyclic receiver graphs before frame reconstruction.
    for _ in 0..source.expression_table.expression_count() {
        match source.expression_table.expression(receiver) {
            ExpressionNode::Borrow(borrow) => receiver = borrow.target,
            ExpressionNode::Member(member) => receiver = member.receiver,
            ExpressionNode::Name(path) => {
                let symbols = source
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                if !path.head_symbol.is_valid()
                    || symbols
                        .first()
                        .is_some_and(|symbol| symbol.is_valid() && *symbol != path.head_symbol)
                {
                    return Err(rejected(
                        "call receiver root lost its resolved source identity",
                    ));
                }
                return Ok(());
            }
            // Computed receivers do not carry a name-path root; their own
            // checked call and operation facts own the resulting value.
            _ => return Ok(()),
        }
    }
    Err(rejected("call receiver path is cyclic"))
}
