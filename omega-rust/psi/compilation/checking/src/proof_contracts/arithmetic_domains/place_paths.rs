//! Canonical place paths and their overlap.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn canonical_path_pair(left: String, right: String) -> (String, String) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

pub(crate) fn place_paths_overlap(left: &str, right: &str) -> bool {
    crate::machine_calls::calls::frame_paths_overlap(left, right)
}

/// The straight-line place path an expression denotes (`self.v`, `count`), for
/// the value environment. `None` for non-place expressions.

/// Both operand expressions' place paths, or `None` when either operand is
/// not a simple place — the common bound-lookup prologue.
pub(crate) fn place_path_pair(
    program: &dyn crate::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(String, String)> {
    let left = place_path(program, left)?;
    let right = place_path(program, right)?;
    Some((left, right))
}

pub(crate) fn place_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.is_empty() {
                return None;
            }
            Some(
                members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
        }
        ExpressionNode::Member(member) => {
            let receiver = place_path(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        _ => None,
    }
}
