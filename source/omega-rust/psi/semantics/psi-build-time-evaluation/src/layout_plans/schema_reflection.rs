//! Exact typed-schema identity and fixed runtime-geometry reflection.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::PrimitiveType;

use super::{SCHEMA_FIELD_CAPACITY, field_key};

#[derive(Debug, Clone)]
pub(crate) struct SchemaFieldInfo {
    pub(crate) name: String,
    pub(crate) identity: Option<u64>,
    pub(crate) key: u64,
    pub(crate) size: u64,
    pub(crate) align: u64,
    pub(crate) source_bits: u64,
    /// Present only for scalar fields. Fixed arrays of primitive elements are
    /// reflected as one aggregate `At` placement and deliberately do not gain
    /// scalar integer/bit/access semantics.
    pub(crate) primitive: Option<PrimitiveType>,
    pub(crate) kind: &'static str,
    pub(crate) declared_range: Option<(i64, i64)>,
    pub(crate) repeated: Option<RepeatedFieldInfo>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RepeatedFieldInfo {
    pub(crate) element_size: u64,
    pub(crate) element_align: u64,
    pub(crate) element_count: u64,
}

pub(crate) fn schema_fields(
    typed: &TypedTrees,
    schema_data: &str,
) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;
    if data.quotient.is_some() {
        return Err(format!(
            "schema reflection cannot observe quotient `{schema_data}`: retained representatives are opaque and require a named lifted operation"
        ));
    }

    let mut fields = Vec::new();
    for member in typed.data_members(data) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        // Reflection describes physical placement demand. Erased bindings stay
        // in the semantic data definition and normalized schema identity, but
        // deliberately receive no field key and no plan entry.
        if field.relevance.is_erased() {
            continue;
        }
        let (size, align, source_bits, primitive, kind, declared_range, repeated) =
            reflected_field_layout(typed, field.type_reference).ok_or_else(|| {
                format!(
                    "schema data `{schema_data}` field `{}` is neither a supported primitive, a fixed array composed of supported primitives, nor a fixed record composed from those shapes",
                    field.name
                )
            })?;
        // A relevant field may itself be a checked record whose complete
        // runtime shape is erased. Keep the field in semantic/schema identity
        // and exact-value checking, but do not manufacture a zero-byte plan
        // entry for it.
        if size == 0 {
            continue;
        }
        let key = field_key(schema_data, field.name.as_str());
        if fields
            .iter()
            .any(|existing: &SchemaFieldInfo| existing.key == key)
        {
            return Err(format!(
                "schema data `{schema_data}` has a compiler field-key collision involving `{}`",
                field.name
            ));
        }
        fields.push(SchemaFieldInfo {
            name: field.name.to_string(),
            identity: field.identity,
            key,
            size,
            align,
            source_bits,
            primitive,
            kind,
            declared_range,
            repeated,
        });
    }
    if fields.is_empty() && typed.data_members(data).is_empty() {
        return Err(format!("schema data `{schema_data}` has no members"));
    }
    if fields.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} fields; the current layout slice supports at most {}",
            fields.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    if data.retired_identities.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} retired identities; reflected Schema supports at most {} per scope",
            data.retired_identities.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    Ok((fields, normalized_schema_identity(typed, data)))
}

pub fn normalized_schema_identity(
    typed: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
) -> u64 {
    psi_typed_trees::identity::normalized_schema_identity(typed, data)
}

fn declared_source_bits(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    primitive: PrimitiveType,
    byte_size: u64,
) -> u64 {
    if primitive == PrimitiveType::Bool {
        return 1;
    }
    let Some(range) = psi_typed_trees::wire::scalar_representation_range(typed, type_reference)
    else {
        return byte_size * 8;
    };
    if range.minimum < 0 {
        return byte_size * 8;
    }
    let maximum = range.maximum as u64;
    u64::from((u64::BITS - maximum.leading_zeros()).max(1))
}

pub(super) fn primitive_byte_size(primitive: PrimitiveType) -> Option<u64> {
    Some(match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
        PrimitiveType::I16 | PrimitiveType::U16 => 2,
        PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32 => 4,
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 => 8,
        _ => return None,
    })
}

type ReflectedFieldLayout = (
    u64,
    u64,
    u64,
    Option<PrimitiveType>,
    &'static str,
    Option<(i64, i64)>,
    Option<RepeatedFieldInfo>,
);

pub(super) fn reflected_field_layout(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<ReflectedFieldLayout> {
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((
            size,
            size,
            declared_source_bits(typed, type_reference, primitive, size),
            Some(primitive),
            "Scalar",
            psi_typed_trees::wire::scalar_representation_range(typed, type_reference)
                .map(|range| (range.minimum, range.maximum)),
            None,
        ));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element_size, element_align) =
                reflected_nested_member_layout(typed, *element_type, &mut Vec::new())?;
            let length = u64::try_from(*length).ok()?;
            let size = element_size.checked_mul(length)?;
            Some((
                size,
                element_align,
                size.checked_mul(8)?,
                None,
                "Repeated",
                None,
                Some(RepeatedFieldInfo {
                    element_size,
                    element_align,
                    element_count: length,
                }),
            ))
        }
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } => {
            let (size, align) =
                reflected_record_layout(typed, *symbol, name.as_str(), &mut Vec::new())?;
            Some((
                size,
                align,
                size.checked_mul(8)?,
                None,
                "Nested",
                None,
                None,
            ))
        }
        _ => None,
    }
}

fn reflected_record_layout(
    typed: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    name: &str,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Option<(u64, u64)> {
    let data = typed.data_definitions().iter().find(|data| {
        if symbol.is_valid() {
            data.symbol == symbol
        } else {
            data.name.as_str() == name
        }
    })?;
    if data.quotient.is_some()
        || data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
        || !data.type_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
        || visiting.contains(&data.symbol)
    {
        return None;
    }
    visiting.push(data.symbol);

    let mut offset = 0u64;
    let mut aggregate_align = 1u64;
    for member in typed.data_members(data) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            visiting.pop();
            return None;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let Some((size, align)) =
            reflected_nested_member_layout(typed, field.type_reference, visiting)
        else {
            visiting.pop();
            return None;
        };
        offset = checked_align_up(offset, align)?.checked_add(size)?;
        aggregate_align = aggregate_align.max(align);
    }
    let result = checked_align_up(offset, aggregate_align).map(|size| (size, aggregate_align));
    visiting.pop();
    result
}

pub(super) fn reflected_nested_member_layout(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
) -> Option<(u64, u64)> {
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((size, size));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element_size, element_align) =
                reflected_nested_member_layout(typed, *element_type, visiting)?;
            Some((
                element_size.checked_mul(u64::try_from(*length).ok()?)?,
                element_align,
            ))
        }
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } => {
            reflected_record_layout(typed, *symbol, name.as_str(), visiting)
        }
        _ => None,
    }
}

pub(super) fn checked_align_up(value: u64, align: u64) -> Option<u64> {
    if align == 0 {
        return None;
    }
    value
        .checked_add(align - 1)
        .map(|value| value / align * align)
}
