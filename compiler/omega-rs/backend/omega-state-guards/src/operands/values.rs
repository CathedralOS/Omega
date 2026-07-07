use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_layout::{DataShape, LayoutPlan};

pub(super) fn resolved_guard_operand_value(
    layouts: &LayoutPlan,
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    match table.expression(expression) {
        ExpressionNode::Boolean(value) => return Some(i64::from(*value)),
        ExpressionNode::Integer(value) => return Some(*value),
        // A float literal resolves to its IEEE-754 bit pattern so a guard like
        // `self.a == 5.0` becomes a CompareStaticValue; the emission compares
        // against these bits via `comisd` (selected by the guard's is_float).
        ExpressionNode::Float(literal) => return Some(literal.value().to_bits() as i64),
        // A CONSTANT float-arith RHS (`self.a == 0.0 - 6.0`) folds to its bits too, so it
        // is a CompareStaticValue like the integer case (`0 - 6` is folded to a literal
        // upstream; float arith is not, so it arrives here as a Binary node). A place
        // operand makes the fold fail -> falls through to the runtime-expression path,
        // which already handles `self.a == self.b + self.c`. The guard's is_float comes
        // from the LEFT place, so a folded-bits RHS still lowers to `ucomisd`.
        ExpressionNode::Binary(_) => {
            if let Some(folded) = const_fold_float(table, expression) {
                return Some(folded.to_bits() as i64);
            }
        }
        _ => {}
    }

    enum_variant_tag_value(layouts, table, expression)
}

/// Folds a guard operand that is a constant f64 expression -- a float literal or a binary
/// arithmetic tree over float literals -- to its value. Returns `None` the moment any leaf
/// is not a constant float (e.g. a place), so a runtime operand is never mistaken for a
/// constant. Strictly constant: no place reads, so the folded value matches the value the
/// arithmetic would produce at runtime.
fn const_fold_float(table: &ExpressionTable, expression: ExpressionHandle) -> Option<f64> {
    match table.expression(expression) {
        ExpressionNode::Float(literal) => Some(literal.value()),
        ExpressionNode::Binary(binary) => {
            let left = const_fold_float(table, binary.left)?;
            let right = const_fold_float(table, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some(left + right),
                BinaryOperator::Subtract => Some(left - right),
                BinaryOperator::Multiply => Some(left * right),
                BinaryOperator::Divide => Some(left / right),
                _ => None,
            }
        }
        _ => None,
    }
}

/// `Some(tag)` when `expression` names a CASE of an enum-shaped data
/// (`Command::Move`). Used both as the guard's static comparison value and to
/// detect tag-only comparisons (the storage operand then reads only the tag).
pub(super) fn enum_variant_tag_value(
    layouts: &LayoutPlan,
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };
    let [_, _] = table.name_path_members(path.members) else {
        return None;
    };
    let type_symbol = path.head_symbol;
    let variant_symbol = path.symbol;
    if !type_symbol.is_valid() || !variant_symbol.is_valid() {
        return None;
    }

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .and_then(|(_, data_layout)| match &data_layout.shape {
            DataShape::Enum { variants, .. } => layouts
                .variants
                .span_or_empty(*variants)
                .iter()
                .position(|candidate| candidate.symbol == variant_symbol)
                .and_then(|index| i64::try_from(index).ok()),
            DataShape::Record { .. } => None,
        })
}
