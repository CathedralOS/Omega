//! Canonical place paths and their overlap.

use crate::frozen_program::expression_table;
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

/// The straight-line place path an expression denotes (`self.v`, `count`), for
/// the value environment. `None` for non-place expressions.
pub(crate) fn place_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    match expression_table(program).expression(expression) {
        ExpressionNode::SavedSubject(saved) => {
            program.saved_value_source(saved)?;
            Some(capture_path(
                expression_table(program)
                    .value_capture(saved.capture)
                    .value_anchor,
            ))
        }
        ExpressionNode::Name(path) => {
            let members = expression_table(program).name_path_members(path.members);
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

/// The element an index expression reads, when its collection is a place and
/// its index is a literal or a named value whose storage can be tracked. Any
/// other index names no trackable element, so it receives no element facts.
pub(super) fn element_place(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<super::value_environment::ElementPlace> {
    use super::ordered_values::Operand;
    let ExpressionNode::Indexed(indexed) = expression_table(program).expression(expression) else {
        return None;
    };
    let collection = place_path(program, indexed.collection)?;
    let index = match expression_table(program).expression(indexed.index) {
        ExpressionNode::Integer(literal) => Operand::Integer(literal.clone(), literal.landing()),
        // Only a value binding names storage whose own writes the operand
        // survival rules see; any other name keeps no element fact.
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.symbol == path.head_symbol
                && matches!(
                    program.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Local
                        | symbols::SymbolKind::Parameter
                        | symbols::SymbolKind::Const
                ) =>
        {
            Operand::Place {
                root: path.symbol,
                path: place_path(program, indexed.index)?,
            }
        }
        _ => return None,
    };
    Some(super::value_environment::ElementPlace { collection, index })
}

/// A value capture is an occurrence, independently of its source's spelling
/// and later storage versions. The private prefix cannot name an authored
/// declaration in this display-path environment.
pub(super) fn capture_path(anchor: ExpressionHandle) -> String {
    format!("\0capture:{anchor:?}")
}
