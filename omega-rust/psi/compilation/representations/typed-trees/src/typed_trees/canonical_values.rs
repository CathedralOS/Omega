//! Replay canonical const values against their independently declared types.

use crate::TypedTrees;
use crate::expression::{ExpressionNode, TableStructLiteral, TableStructLiteralField};
use crate::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use language_semantics::const_value::DecodedCanonicalConstValue;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};

pub(crate) fn materialize(
    program: &mut TypedTrees,
    declared_type: TypeReferenceHandle,
    value: &DecodedCanonicalConstValue,
) -> Option<ExpressionNode> {
    match value {
        // Declaration receipts may contain floats; generic indices may not.
        DecodedCanonicalConstValue::Float { .. } => None,
        DecodedCanonicalConstValue::Integer { type_name, value } => {
            let primitive = program.type_reference_table.primitive_type(declared_type)?;
            if type_name != primitive.name() {
                return None;
            }
            let literal = IntegerLiteral::from_parts(
                *value < 0,
                IntegerRadix::Decimal,
                &value.unsigned_abs().to_string(),
            )
            .ok()?;
            integer_value(program, declared_type, literal)
        }
        DecodedCanonicalConstValue::Boolean(value) => {
            (program.type_reference_table.primitive_type(declared_type)
                == Some(PrimitiveType::Bool))
            .then_some(ExpressionNode::Boolean(*value))
        }
        DecodedCanonicalConstValue::Array { type_name, values } => {
            if *type_name != program.display_type_reference(declared_type) {
                return None;
            }
            let declared_type = program.unwrapped_type_reference(declared_type)?;
            let TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } = program
                .type_reference_table
                .type_reference(declared_type)
                .clone()
            else {
                return None;
            };
            if length != values.len() {
                return None;
            }
            let mut elements = Vec::with_capacity(length);
            for value in values {
                let element = materialize(program, element_type, value)?;
                elements.push(program.expression_table.insert(element));
            }
            Some(ExpressionNode::ArrayLiteral(
                program.expression_table.insert_expression_handles(elements),
            ))
        }
        DecodedCanonicalConstValue::Record { type_name, fields } => {
            record(program, declared_type, type_name, None, fields)
        }
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } => record(program, declared_type, type_name, Some(case_name), fields),
    }
}

fn record(
    program: &mut TypedTrees,
    declared_type: TypeReferenceHandle,
    type_name: &str,
    case_name: Option<&str>,
    fields: &[(String, DecodedCanonicalConstValue)],
) -> Option<ExpressionNode> {
    let declared_type = program.unwrapped_type_reference(declared_type)?;
    let symbol = &program
        .type_reference_table
        .type_reference(declared_type)
        .as_named_symbol()?;
    // Encoded names are consistency claims, never a route to another carrier.
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol.is_valid() && definition.symbol == *symbol)?;
    if definition.name.as_str() != type_name {
        return None;
    }
    let mut literal = TableStructLiteral {
        type_name: definition.name.clone(),
        type_symbol: definition.symbol,
        ..TableStructLiteral::default()
    };
    let declared_fields = if let Some(case_name) = case_name {
        let variant = program.data_members(definition).iter().find_map(|member| {
            let variant = member.as_variant()?;
            (variant.name.as_str() == case_name).then_some(variant)
        })?;
        literal.case_name = Some(variant.name.clone());
        literal.case_symbol = Some(variant.symbol);
        program.data_payload_fields(variant).to_vec()
    } else {
        program
            .data_members(definition)
            .iter()
            .map(|member| {
                let field = member.as_field()?;
                Some(field.clone())
            })
            .collect::<Option<Vec<_>>>()?
    };
    if declared_fields.len() != fields.len() {
        return None;
    }
    let mut materialized = Vec::with_capacity(fields.len());
    for (declared, (name, value)) in declared_fields.iter().zip(fields) {
        if declared.name.as_str() != name || !declared.symbol.is_valid() {
            return None;
        }
        let value = materialize(program, declared.type_reference, value)?;
        materialized.push(TableStructLiteralField {
            name: declared.name.clone(),
            field_symbol: declared.symbol,
            value: program.expression_table.insert(value),
        });
    }
    literal.fields = program.expression_table.insert_struct_fields(materialized);
    Some(ExpressionNode::StructLiteral(literal))
}

fn integer_value(
    program: &TypedTrees,
    declared_type: TypeReferenceHandle,
    literal: IntegerLiteral,
) -> Option<ExpressionNode> {
    let primitive = program.type_reference_table.primitive_type(declared_type)?;
    let landed_type = primitive.landed_landing()?;
    Some(ExpressionNode::Integer(
        literal.with_landing(IntegerLanding {
            landed_type,
            domain: program
                .type_reference_table
                .arithmetic_domain(declared_type),
        }),
    ))
}

impl TypedTrees {
    /// Schema-directed representation construction, not const admission or
    /// execution. The caller supplies an independently resolved carrier and
    /// checks the value's eligibility, predicates and selected source custody.
    pub fn materialize_canonical_const_value(
        &mut self,
        carrier: TypeReferenceHandle,
        value: &DecodedCanonicalConstValue,
    ) -> Option<ExpressionNode> {
        materialize(self, carrier, value)
    }
}
