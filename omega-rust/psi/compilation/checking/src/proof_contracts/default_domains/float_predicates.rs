//! Evaluate landed float predicates against an explicit field valuation.
//! Declaration admission and invariant windows share IEEE meaning, but supply
//! different snapshots. Missing fields and unsupported operations stay unknown;
//! neither source spelling nor an integer interval grants float algebra.

use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Judge a data predicate at the caller's exact current field values. The
/// callback supplies meaning in the declared primitive format, never a
/// decimal approximation. This proves only the expression: membership,
/// containment, first-case validity and binding custody remain independent.
pub fn data_float_predicate(
    program: &TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
) -> Option<bool> {
    predicate(program, definition, expression, field_value, false, 0)
}

/// Prove the complete expression predicate of a closed record at zero.
/// Early callers may precede literal stamping: an anonymous endpoint takes
/// its format only from the exact opposite field operand, with the same
/// selected comparison authority checked afterward. This does not establish
/// contained fields, memberships, cases or binding custody.
pub fn data_predicates_admit_zero(program: &TypedTrees, definition: &DataDefinition) -> bool {
    if !definition.type_parameters.is_empty()
        || definition.where_facts.is_empty()
        || program
            .data_members(definition)
            .iter()
            .any(|member| matches!(member, DataMember::Variant(_)))
    {
        return false;
    }
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .all(|fact| {
            let ProofFact::Expression(expression) = fact else {
                return false;
            };
            predicate(
                program,
                definition,
                *expression,
                &|_, _| Some(FloatMeaning::Zero { negative: false }),
                true,
                0,
            ) == Some(true)
        })
}

fn predicate(
    program: &TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
    infer_field_landing: bool,
    depth: usize,
) -> Option<bool> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            _ => None,
        };
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        // Evaluate both clauses: no unknown obligation is discharged merely
        // because another clause short-circuits its runtime evaluation.
        let left = predicate(
            program,
            definition,
            binary.left,
            field_value,
            infer_field_landing,
            depth + 1,
        )?;
        let right = predicate(
            program,
            definition,
            binary.right,
            field_value,
            infer_field_landing,
            depth + 1,
        )?;
        return Some(if binary.operator == BinaryOperator::And {
            left && right
        } else {
            left || right
        });
    }
    let field_format = |expression| {
        infer_field_landing
            .then(|| {
                let field = declared_field(program, definition, expression)?;
                match program.primitive_type_reference(field.type_reference)? {
                    PrimitiveType::F32 => Some(numerics::literals::FloatFormat::F32),
                    PrimitiveType::F64 => Some(numerics::literals::FloatFormat::F64),
                    _ => None,
                }
            })
            .flatten()
    };
    let (left, left_primitive, left_type) = operand(
        program,
        definition,
        binary.left,
        field_value,
        field_format(binary.right),
    )?;
    let (right, right_primitive, right_type) = operand(
        program,
        definition,
        binary.right,
        field_value,
        field_format(binary.left),
    )?;
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
    program: &TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
    field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
    destination: Option<numerics::literals::FloatFormat>,
) -> Option<(FloatMeaning, PrimitiveType, TypeReferenceHandle)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
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
    program: &'program TypedTrees,
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == path.symbol).then_some(field)
    })
}

#[cfg(test)]
mod tests;
