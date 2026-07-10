//! R1 dependent-range recognizer (chapter 12): the ADMISSIBLE symbolic
//! bound shapes a declared range's endpoint may take. One recognizer, three
//! policy consumers -- the validation fence (which non-constant bounds are
//! legal, omega-validation type_references.rs), the proof-plan atom minting
//! (omega-proof obligations.rs), and the callee-side range substitution
//! (omega-typed-trees-to-checked-trees index proofs) -- so "admissible" can
//! never drift between the gate and the dischargers.
//!
//! Rung R1a admits exactly `self.<field>` plus an optional literal offset:
//! `[0..=self.count]` -> (count, 0); the exclusive sugar `[0..self.count]`
//! parses as `self.count - 1` -> (count, -1). Everything else stays behind
//! the non-constant-bound fence.

use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::name::Identifier;

/// The recognized symbolic maximum: the named `self` FIELD and a literal
/// offset applied to its entry value (`self.count - 1` -> offset -1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaxBound {
    pub field: Identifier,
    pub offset: i64,
}

/// Recognizes an R1a-admissible symbolic bound expression. `None` means the
/// bound is not in the admissible class (callers keep their literal path or
/// their fence -- never treat `None` as unbounded).
pub fn symbolic_max_bound(
    table: &ExpressionTable,
    bound: ExpressionHandle,
) -> Option<SymbolicMaxBound> {
    if !bound.is_valid() {
        return None;
    }
    match table.expression(bound) {
        ExpressionNode::Member(_) => {
            let field = self_field_name(table, bound)?;
            Some(SymbolicMaxBound { field, offset: 0 })
        }
        ExpressionNode::Binary(binary) => {
            let field = self_field_name(table, binary.left)?;
            let ExpressionNode::Integer(literal) = table.expression(binary.right) else {
                return None;
            };
            let magnitude = literal.value_i64()?;
            let offset = match binary.operator {
                BinaryOperator::Add => magnitude,
                BinaryOperator::Subtract => magnitude.checked_neg()?,
                _ => return None,
            };
            Some(SymbolicMaxBound { field, offset })
        }
        _ => None,
    }
}

/// `self.<field>` (a Member whose receiver is the bare `self` name), or
/// `None` for any other shape -- locals, params, and deeper chains are not
/// in the R1a class (a field's range is store-enforced machine-wide, which
/// is what makes the substitution in the callee sound).
fn self_field_name(table: &ExpressionTable, expression: ExpressionHandle) -> Option<Identifier> {
    let ExpressionNode::Member(member) = table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Name(path) = table.expression(member.receiver) else {
        return None;
    };
    let [only] = table.name_path_members(path.members) else {
        return None;
    };
    (only.as_str() == "self").then(|| member.member.clone())
}
