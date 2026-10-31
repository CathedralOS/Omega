//! Evaluate landed float predicates against an explicit field valuation.
//! Declaration admission and invariant windows share IEEE meaning, but supply
//! different snapshots. Missing fields and unsupported operations stay unknown;
//! neither source spelling nor an integer interval grants float algebra.

use crate::frozen_program::expression_table;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::data::{DataDefinition, DataField};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Judge a data predicate at the caller's exact current field values. The
/// callback supplies meaning in the declared primitive format, never a
/// decimal approximation. This proves only the expression: membership,
/// containment, first-case validity and binding custody remain independent.
pub fn data_float_predicate(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
) -> Option<bool> {
    predicate(program, definition, expression, field_value, 0)
}

/// The exact IEEE meaning of the literal in a `field <comparison> literal`
/// conjunct of `definition`'s facts, read at the field's own format. The
/// literal lands at that format exactly as the predicate itself does, and the
/// comparison must select the builtin or sealed float meaning; a user-supplied
/// comparison states no interval. `None` for any other conjunct shape.
pub(crate) fn float_field_literal_bound_meaning(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    conjunct: ExpressionHandle,
) -> Option<FloatMeaning> {
    if !expression_table(program).expression_is_valid(conjunct) {
        return None;
    }
    let Some(binary) = expression_table(program).binary_node(conjunct) else {
        return None;
    };
    let field_format = |expression| {
        let field = declared_field(program, definition, expression)?;
        match program.primitive_type_reference(field.type_reference)? {
            PrimitiveType::F32 => Some(numerics::literals::FloatFormat::F32),
            PrimitiveType::F64 => Some(numerics::literals::FloatFormat::F64),
            _ => None,
        }
    };
    // Field operands supply no value here; only the literal's meaning is read.
    let unread = |_, _| Some(FloatMeaning::Zero { negative: false });
    let (left, left_primitive, left_type) = operand(
        program,
        definition,
        binary.left,
        &unread,
        field_format(binary.right),
    )?;
    let (right, right_primitive, right_type) = operand(
        program,
        definition,
        binary.right,
        &unread,
        field_format(binary.left),
    )?;
    if left_primitive != right_primitive
        || !crate::has_builtin_or_sealed_float_comparison_meaning(
            program,
            definition.symbol,
            conjunct,
            binary.operator,
            &[Some(left_type), Some(right_type)],
        )
    {
        return None;
    }
    match (
        expression_table(program).expression(binary.left),
        expression_table(program).expression(binary.right),
    ) {
        (ExpressionNode::Float(_), ExpressionNode::Name(_)) => Some(left),
        (ExpressionNode::Name(_), ExpressionNode::Float(_)) => Some(right),
        _ => None,
    }
}

fn predicate(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
    depth: usize,
) -> Option<bool> {
    let node = expression_table(program).expression_at_depth(expression, depth)?;
    let ExpressionNode::Binary(binary) = node else {
        return match expression_table(program).expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            _ => None,
        };
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        // Evaluate both clauses: no unknown obligation is discharged merely
        // because another clause short-circuits its runtime evaluation.
        let left = predicate(program, definition, binary.left, field_value, depth + 1)?;
        let right = predicate(program, definition, binary.right, field_value, depth + 1)?;
        return Some(if binary.operator == BinaryOperator::And {
            left && right
        } else {
            left || right
        });
    }
    let (left, left_primitive, left_type) =
        operand(program, definition, binary.left, field_value, None)?;
    let (right, right_primitive, right_type) =
        operand(program, definition, binary.right, field_value, None)?;
    if left_primitive != right_primitive
        || !crate::has_builtin_or_sealed_float_comparison_meaning(
            program,
            definition.symbol,
            expression,
            binary.operator,
            &[Some(left_type), Some(right_type)],
        )
    {
        return None;
    }
    Some(match binary.operator {
        BinaryOperator::Equal => FloatSemantics::equal(&left, &right),
        BinaryOperator::NotEqual => FloatSemantics::not_equal(&left, &right),
        BinaryOperator::Less => FloatSemantics::less(&left, &right),
        BinaryOperator::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        BinaryOperator::Greater => FloatSemantics::greater(&left, &right),
        BinaryOperator::GreaterOrEqual => FloatSemantics::greater_or_equal(&left, &right),
        _ => return None,
    })
}

fn operand(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
    destination: Option<numerics::literals::FloatFormat>,
) -> Option<(FloatMeaning, PrimitiveType, TypeReferenceHandle)> {
    match expression_table(program).expression(expression) {
        ExpressionNode::Name(path) if crate::is_single_segment_name_path(program, path) => {
            let field = declared_field(program, definition, expression)?;
            let primitive = program.primitive_type_reference(field.type_reference)?;
            if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                return None;
            }
            Some((
                field_value(field.symbol, primitive)?,
                primitive,
                field.type_reference,
            ))
        }
        ExpressionNode::Float(literal) => {
            let (format, primitive, atom) = match literal.landing().or(destination)? {
                numerics::literals::FloatFormat::F32 => (
                    FloatFormat::BINARY32,
                    PrimitiveType::F32,
                    BuiltinTypeAtom::F32,
                ),
                numerics::literals::FloatFormat::F64 => (
                    FloatFormat::BINARY64,
                    PrimitiveType::F64,
                    BuiltinTypeAtom::F64,
                ),
            };
            let symbol = program
                .symbols
                .child_handles(program.symbols.root())?
                .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
            let type_reference = program
                .type_reference_table
                .find_named_type_reference(symbol)?;
            Some((
                FloatSemantics::from_decimal(format, literal.text())?,
                primitive,
                type_reference,
            ))
        }
        _ => None,
    }
}

fn declared_field<'program>(
    program: &'program dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    let symbol = crate::expression_traversal::single_segment_name_symbol(program, expression)?;
    program.data_field(definition, symbol)
}
