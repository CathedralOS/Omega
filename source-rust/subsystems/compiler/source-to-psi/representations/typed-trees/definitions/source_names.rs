//! Resolved names retain their exact declaration and lexical source scope.
//! These queries explain missing source bindings; they grant no body facts.

use crate::source_state::TypedTrees;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};

pub fn receiver_member_chain(
    program: &TypedTrees,
    receiver: ExpressionHandle,
) -> Option<Vec<String>> {
    if !program.expression_table.expression_is_valid(receiver) {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_owned());
            Some(chain)
        }
        ExpressionNode::Borrow(borrow) => receiver_member_chain(program, borrow.target),
        _ => None,
    }
}
