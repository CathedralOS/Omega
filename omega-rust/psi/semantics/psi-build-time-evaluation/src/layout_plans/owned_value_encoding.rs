//! Recursive type-directed encoding for source-owned fixed materialization.

use psi_layout_plans::{ByteOrder, MaterializationDiagnostic};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{
    BuildTimeValue, checked_align_up, primitive_byte_size, reflected_nested_member_layout,
};

pub(super) fn exact_struct_fields<'a>(
    type_name: &str,
    fields: &'a [(String, BuildTimeValue)],
) -> Result<std::collections::BTreeMap<&'a str, &'a BuildTimeValue>, MaterializationDiagnostic> {
    let mut supplied = std::collections::BTreeMap::new();
    for (name, value) in fields {
        if supplied.insert(name.as_str(), value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "typed owned value `{type_name}` supplies field `{name}` more than once"
            )));
        }
    }
    Ok(supplied)
}

pub(super) fn encode_typed_owned_value(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<Vec<u8>, MaterializationDiagnostic> {
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let width = primitive_byte_size(primitive).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "typed owned materialization does not support primitive `{}`",
                primitive.name()
            ))
        })?;
        if let BuildTimeValue::Int(value) = value {
            let in_carrier_range = match primitive {
                PrimitiveType::I8 => i8::try_from(*value).is_ok(),
                PrimitiveType::U8 => u8::try_from(*value).is_ok(),
                PrimitiveType::I16 => i16::try_from(*value).is_ok(),
                PrimitiveType::U16 => u16::try_from(*value).is_ok(),
                PrimitiveType::I32 => i32::try_from(*value).is_ok(),
                PrimitiveType::U32 => u32::try_from(*value).is_ok(),
                PrimitiveType::I64 => true,
                PrimitiveType::U64 => *value >= 0,
                _ => false,
            };
            let in_declared_range =
                psi_typed_trees::wire::scalar_representation_range(typed, type_reference)
                    .is_none_or(|range| *value >= range.minimum && *value <= range.maximum);
            if !in_carrier_range || !in_declared_range {
                return Err(MaterializationDiagnostic(format!(
                    "typed integer value {value} is outside `{}`",
                    typed.display_type_reference(type_reference)
                )));
            }
        }
        let bits = match (primitive, value) {
            (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => u64::from(*value),
            (PrimitiveType::F32, BuildTimeValue::Float(value)) => {
                u64::from((*value as f32).to_bits())
            }
            (PrimitiveType::F64, BuildTimeValue::Float(value)) => value.to_bits(),
            (
                PrimitiveType::I8
                | PrimitiveType::U8
                | PrimitiveType::I16
                | PrimitiveType::U16
                | PrimitiveType::I32
                | PrimitiveType::U32
                | PrimitiveType::I64
                | PrimitiveType::U64,
                BuildTimeValue::Int(value),
            ) => *value as u64,
            _ => {
                return Err(MaterializationDiagnostic(format!(
                    "typed owned value does not match primitive `{}`",
                    primitive.name()
                )));
            }
        };
        let bytes = match byte_order {
            ByteOrder::LittleEndian => bits.to_le_bytes(),
            ByteOrder::BigEndian => bits.to_be_bytes(),
        };
        let width = usize::try_from(width).expect("supported primitive width fits usize");
        return Ok(match byte_order {
            ByteOrder::LittleEndian => bytes[..width].to_vec(),
            ByteOrder::BigEndian => bytes[bytes.len() - width..].to_vec(),
        });
    }

    match typed.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err(MaterializationDiagnostic(
                    "typed owned fixed-array value is not an array".into(),
                ));
            };
            if elements.len() != *length {
                return Err(MaterializationDiagnostic(format!(
                    "typed owned fixed array has {} elements, expected {length}",
                    elements.len()
                )));
            }
            let mut bytes = Vec::new();
            for element in elements {
                bytes.extend(encode_typed_owned_value(
                    typed,
                    *element_type,
                    element,
                    byte_order,
                    visiting,
                )?);
            }
            Ok(bytes)
        }
        TypeReferenceNode::Named { symbol, name } => {
            encode_typed_owned_record(typed, *symbol, name.as_str(), value, byte_order, visiting)
        }
        _ => Err(MaterializationDiagnostic(
            "typed owned value is outside the supported fixed aggregate subset".into(),
        )),
    }
}

fn encode_typed_owned_record(
    typed: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    name: &str,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<Vec<u8>, MaterializationDiagnostic> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| {
            if symbol.is_valid() {
                data.symbol == symbol
            } else {
                data.name.as_str() == name
            }
        })
        .ok_or_else(|| MaterializationDiagnostic(format!("no typed data named `{name}` exists")))?;
    if visiting.contains(&data.symbol) {
        return Err(MaterializationDiagnostic(format!(
            "typed owned record `{name}` is recursively laid out"
        )));
    }
    if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
        || !data.type_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
    {
        return Err(MaterializationDiagnostic(format!(
            "typed owned record `{name}` is outside the fixed checked-shape subset"
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value for `{name}` is not a record"
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned nested value `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let supplied = exact_struct_fields(name, fields)?;
    let members = typed.data_members(data);
    if members
        .iter()
        .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
    {
        return Err(MaterializationDiagnostic(format!(
            "typed owned record `{name}` contains a sum case"
        )));
    }
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned record `{name}` has {} fields, expected {}",
            supplied.len(),
            members.len()
        )));
    }
    for member in members {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            unreachable!("sum cases rejected above")
        };
        if !supplied.contains_key(field.name.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "typed owned record `{name}` has no field `{}`",
                field.name
            )));
        }
    }

    visiting.push(data.symbol);
    let result = (|| {
        let mut bytes = Vec::new();
        let mut aggregate_align = 1u64;
        for member in members {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                unreachable!("record subset validated above")
            };
            if field.relevance.is_erased() {
                continue;
            }
            let (_, align) = reflected_nested_member_layout(typed, field.type_reference, visiting)
                .ok_or_else(|| {
                    MaterializationDiagnostic(format!(
                        "typed field `{}` is outside the fixed aggregate subset",
                        field.name
                    ))
                })?;
            let aligned = checked_align_up(bytes.len() as u64, align).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "typed field `{}` alignment overflows",
                    field.name
                ))
            })?;
            bytes.resize(
                usize::try_from(aligned).map_err(|_| {
                    MaterializationDiagnostic("typed aggregate extent exceeds compiler host".into())
                })?,
                0,
            );
            let field_value = supplied.get(field.name.as_str()).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "typed owned record `{name}` has no field `{}`",
                    field.name
                ))
            })?;
            bytes.extend(encode_typed_owned_value(
                typed,
                field.type_reference,
                field_value,
                byte_order,
                visiting,
            )?);
            aggregate_align = aggregate_align.max(align);
        }
        let size = checked_align_up(bytes.len() as u64, aggregate_align).ok_or_else(|| {
            MaterializationDiagnostic(format!("typed record `{name}` extent overflows"))
        })?;
        bytes.resize(
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic("typed aggregate extent exceeds compiler host".into())
            })?,
            0,
        );
        Ok(bytes)
    })();
    visiting.pop();
    result
}
