use omega_layout::{DataShape, LayoutPlan};
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_numerics::float_semantics::{
    FloatFormat as SemanticFloatFormat, FloatMeaning, FloatSemantics,
};

/// Whether a guard operand is a CONSTANT float expression (a float literal
/// or foldable float arithmetic) -- the conjunction clause builder marks
/// such compares float-kinded so the emission takes the FCMP path.
pub(crate) fn guard_operand_is_float_constant(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Float(_) => true,
        ExpressionNode::Binary(_) => const_fold_float(table, expression).is_some(),
        _ => false,
    }
}

pub(super) fn resolved_guard_operand_value(
    layouts: &LayoutPlan,
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    match table.expression(expression) {
        ExpressionNode::Boolean(value) => return Some(i64::from(*value)),
        // The bits fallback serves u64-magnitude literals (D14 fire F): the
        // literal-width gate only blesses them into EQUALITY guards against
        // u64-classed places, where the 8-byte two's-complement pattern IS
        // the value under bit-pattern compare; an ordering guard never sees
        // an oversize literal (refused at validation), so the signed compare
        // the encoder emits stays sound.
        ExpressionNode::Integer(value) => {
            return value
                .value_i64()
                .or_else(|| value.bits_u64().map(|bits| bits as i64));
        }
        // A float literal resolves to its IEEE-754 bit pattern so a guard like
        // `self.a == 5.0` becomes a CompareStaticValue; the emission compares
        // against these bits via `comisd` (selected by the guard's is_float).
        ExpressionNode::Float(literal) => return Some(literal.landed_f64().to_bits() as i64),
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

/// Folds a guard operand that is a constant float expression -- a float literal or a
/// binary arithmetic tree over float literals -- to its value, PER-OP at the tree's
/// LANDED width (ch5 / float ladder F2c): an F32-stamped tree (the comparison adopted
/// an f32 place's format at validation) rounds every operation to f32, exactly as the
/// runtime f32 ops would -- folding at a raw f64 window diverges at the f32
/// precision cliff (2^24 + 1.0). The fold consumes the shared executable
/// `FloatSemantics` definition rather than host arithmetic, keeping this
/// backend-time consumer identical to landed interpreter evaluation. Returns
/// `None` the moment any leaf is not a constant float (e.g. a place), so a
/// runtime operand is never mistaken for a constant. Strictly constant: no
/// place reads, so the folded value matches the runtime value.
fn const_fold_float(table: &ExpressionTable, expression: ExpressionHandle) -> Option<f64> {
    let format = match tree_float_landing(table, expression) {
        Some(psi_numerics::literals::FloatFormat::F32) => SemanticFloatFormat::BINARY32,
        Some(psi_numerics::literals::FloatFormat::F64) | None => SemanticFloatFormat::BINARY64,
    };
    Some(const_fold_float_at(table, expression, format)?.to_interpreter_value(format))
}

/// The tree's landed format witness: the first landed float literal (left first),
/// mirroring the operand-derived landing law. `None` = the anonymous f64 window.
fn tree_float_landing(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<psi_numerics::literals::FloatFormat> {
    match table.expression(expression) {
        ExpressionNode::Float(literal) => literal.landing(),
        ExpressionNode::Binary(binary) => tree_float_landing(table, binary.left)
            .or_else(|| tree_float_landing(table, binary.right)),
        _ => None,
    }
}

fn const_fold_float_at(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    format: SemanticFloatFormat,
) -> Option<FloatMeaning> {
    match table.expression(expression) {
        ExpressionNode::Float(literal) => {
            let value = literal.landed_f64();
            Some(if format == SemanticFloatFormat::BINARY32 {
                FloatSemantics::convert(format, &FloatMeaning::from_f64(value))
            } else {
                FloatMeaning::from_f64(value)
            })
        }
        ExpressionNode::Binary(binary) => {
            let left = const_fold_float_at(table, binary.left, format)?;
            let right = const_fold_float_at(table, binary.right, format)?;
            match binary.operator {
                BinaryOperator::Add => Some(FloatSemantics::add(format, &left, &right)),
                BinaryOperator::Subtract => Some(FloatSemantics::subtract(format, &left, &right)),
                BinaryOperator::Multiply => Some(FloatSemantics::multiply(format, &left, &right)),
                BinaryOperator::Divide => Some(FloatSemantics::divide(format, &left, &right)),
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
