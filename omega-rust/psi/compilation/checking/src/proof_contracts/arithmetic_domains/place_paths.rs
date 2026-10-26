//! Canonical place paths and their overlap.

use symbols::JoinedPath;
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
        ExpressionNode::SavedSubject(saved) => {
            program.saved_value_source(saved)?;
            Some(capture_path(
                program
                    .expression_table
                    .value_capture(saved.capture)
                    .value_anchor,
            ))
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.is_empty() {
                return None;
            }
            Some(members.joined_path_with("."))
        }
        ExpressionNode::Member(member) => {
            let receiver = place_path(program, member.receiver)?;
            Some(format!("{receiver}.{}", member.member.as_str()))
        }
        _ => None,
    }
}

/// A value capture is an occurrence, independently of its source's spelling
/// and later storage versions. The private prefix cannot name an authored
/// declaration in this display-path environment.
pub(super) fn capture_path(anchor: ExpressionHandle) -> String {
    format!("\0capture:{anchor:?}")
}
