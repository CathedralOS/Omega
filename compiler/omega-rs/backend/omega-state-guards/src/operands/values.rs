use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_core::literals::FloatFormat;
use omega_layout::{DataShape, LayoutPlan};

/// Whether a guard operand is a CONSTANT float expression (a float literal
/// or foldable float arithmetic) -- the conjunction clause builder marks
/// such compares float-kinded so the emission takes the FCMP path.
pub(crate) fn guard_operand_is_float_constant(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match table.expression(expression) {
        ExpressionNode::Float(_) => true,
        ExpressionNode::Binary(_) => const_fold_float(table, expression, None).is_some(),
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
            if let Some(folded) = const_fold_float(table, expression, None) {
                return Some(folded.to_bits() as i64);
            }
        }
        _ => {}
    }

    enum_variant_tag_value(layouts, table, expression)
}

/// Folds a constant float tree at its explicit or contextual format. Each
/// arithmetic node rounds at that width, matching the runtime instruction;
/// a place leaf still refuses the fold.
fn const_fold_float(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    contextual_format: Option<FloatFormat>,
) -> Option<f64> {
    // Anonymous constants remain exact Rat trees until this fold site requests
    // a format. Round the whole anonymous subtree once; landed leaves take the
    // per-operation path below instead.
    if let Some(exact) = exact_anonymous_float_expression(table, expression) {
        return Some(match contextual_format.unwrap_or(FloatFormat::F64) {
            FloatFormat::F32 => f64::from(exact.to_f32()),
            FloatFormat::F64 => exact.to_f64(),
        });
    }
    let format = float_expression_format(table, expression).or(contextual_format);
    match table.expression(expression) {
        ExpressionNode::Float(literal) => Some(match format {
            Some(FloatFormat::F32) => f64::from(literal.value_f32()),
            Some(FloatFormat::F64) | None => literal.landed_f64(),
        }),
        ExpressionNode::Binary(binary) => {
            let left = const_fold_float(table, binary.left, format)?;
            let right = const_fold_float(table, binary.right, format)?;
            Some(match format {
                Some(FloatFormat::F32) => {
                    let (left, right) = (left as f32, right as f32);
                    f64::from(match binary.operator {
                        BinaryOperator::Add => left + right,
                        BinaryOperator::Subtract => left - right,
                        BinaryOperator::Multiply => left * right,
                        BinaryOperator::Divide => left / right,
                        _ => return None,
                    })
                }
                Some(FloatFormat::F64) | None => match binary.operator {
                    BinaryOperator::Add => left + right,
                    BinaryOperator::Subtract => left - right,
                    BinaryOperator::Multiply => left * right,
                    BinaryOperator::Divide => left / right,
                    _ => return None,
                },
            })
        }
        _ => None,
    }
}

fn exact_anonymous_float_expression(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<omega_core::bignum::ExactFloat> {
    use omega_core::bignum::ExactFloat;

    match table.expression(expression) {
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Binary(binary) => {
            let left = exact_anonymous_float_expression(table, binary.left)?;
            let right = exact_anonymous_float_expression(table, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.mul(&right),
                BinaryOperator::Divide => left.div(&right),
                _ => return None,
            })
        }
        _ => None,
    }
}

/// Re-fold a constant guard leg at the width of the place on the other side.
/// Anonymous literals inherit that width; an explicit f64 suffix promotes
/// the expression and keeps an f64 fold.
pub(super) fn resolved_float_guard_operand_value(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    place_byte_size: usize,
) -> Option<i64> {
    let contextual_format = match place_byte_size {
        4 => FloatFormat::F32,
        8 => FloatFormat::F64,
        _ => return None,
    };
    const_fold_float(table, expression, Some(contextual_format)).map(|value| value.to_bits() as i64)
}

fn float_expression_format(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<FloatFormat> {
    match table.expression(expression) {
        ExpressionNode::Float(literal) => literal.landing(),
        ExpressionNode::Binary(binary) => {
            let left = float_expression_format(table, binary.left);
            let right = float_expression_format(table, binary.right);
            match (left, right) {
                (Some(FloatFormat::F64), _) | (_, Some(FloatFormat::F64)) => Some(FloatFormat::F64),
                (Some(FloatFormat::F32), _) | (_, Some(FloatFormat::F32)) => Some(FloatFormat::F32),
                (None, None) => None,
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
