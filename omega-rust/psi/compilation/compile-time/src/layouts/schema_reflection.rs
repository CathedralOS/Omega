//! Exact typed-schema identity and fixed runtime-geometry reflection.

use typed_trees::TypedTrees;
use typed_trees::types::PrimitiveType;

mod schema_value;
pub(crate) use schema_value::{build_schema_value, build_selected_schema_value};

const SCHEMA_FIELD_CAPACITY: usize = 32;

pub(crate) fn local_schema_field_discriminator(schema: &str, field: &str) -> u64 {
    // Stable FNV-1a. This value is only a compiler-policy-local lookup key:
    // every reflected scope rejects collisions before exposing it, and exact
    // schema structure supplies durable identity. Zero remains the unused-tail
    // sentinel.
    let mut hash = crate::fnv::OFFSET_BASIS;
    for byte in schema.bytes().chain(*b"::").chain(field.bytes()) {
        crate::fnv::byte(&mut hash, byte);
    }
    if hash == 0 { 1 } else { hash }
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaFieldInfo {
    pub(crate) schema_field: symbols::SymbolHandle,
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
        .data_definition_by_name_str(schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;
    schema_fields_for(typed, data)
}

pub(crate) fn schema_fields_for(
    typed: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
    let schema_data = data.name.as_str();
    require_invariant_schema(typed, data.symbol, &[])?;
    if data.quotient.is_some() {
        return Err(format!(
            "schema reflection cannot observe quotient `{schema_data}`: retained representatives are opaque and require a named lifted operation"
        ));
    }

    let mut fields = Vec::new();
    for member in typed.data_members(data) {
        let Some(field) = member.as_field() else {
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
        let key = local_schema_field_discriminator(schema_data, field.name.as_str());
        if fields
            .iter()
            .any(|existing: &SchemaFieldInfo| existing.key == key)
        {
            return Err(format!(
                "schema data `{schema_data}` has a compiler-local field discriminator collision involving `{}`",
                field.name
            ));
        }
        fields.push(SchemaFieldInfo {
            schema_field: field.symbol,
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
    Ok((fields, normalized_schema_report_fingerprint(typed, data)))
}

/// Schema arguments must have one physical meaning before a pure policy runs.
/// Pending family applications are supplied by the installer because none of
/// that batch's results has entered typed storage yet.
pub(crate) fn require_invariant_schema(
    typed: &TypedTrees,
    schema: symbols::SymbolHandle,
    pending: &[symbols::SymbolHandle],
) -> Result<(), String> {
    require_schema_layouts(typed, schema, pending, "target-dependent")
}

/// A pending ordinary policy is not ordinary packing. Family fold inputs may
/// observe it only after its exact invariant geometry has been installed.
pub(crate) fn require_materialized_schema(
    typed: &TypedTrees,
    schema: symbols::SymbolHandle,
    pending: &[symbols::SymbolHandle],
) -> Result<(), String> {
    require_schema_layouts(typed, schema, pending, "unmaterialized policy")
}

fn require_schema_layouts(
    typed: &TypedTrees,
    schema: symbols::SymbolHandle,
    pending: &[symbols::SymbolHandle],
    pending_kind: &str,
) -> Result<(), String> {
    let mut records = vec![schema];
    let mut visited = Vec::new();
    while let Some(symbol) = records.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        if pending.contains(&symbol) {
            return Err(format!(
                "schema contains {pending_kind} layout `{}`; nested schema arguments require exact retained invariant geometry",
                typed.symbols.name(symbol)
            ));
        }
        if typed.plan_laid_layout(symbol).is_some_and(|layout| {
            matches!(
                layout.realizations,
                typed_trees::PlanLaidRealizations::TargetFamily(_)
            )
        }) {
            return Err(format!(
                "schema contains target-dependent layout `{}`; nested target-dependent schema arguments are not yet supported",
                typed.symbols.name(symbol)
            ));
        }
        let Some(data) = typed.data_definition_by_symbol(symbol) else {
            continue;
        };
        let mut members = typed
            .live_data_fields(typed.data_members(data))
            .map(|field| field.type_reference)
            .collect::<Vec<_>>();
        while let Some(member) = members.pop() {
            use typed_trees::types::TypeReferenceNode;
            match typed.type_reference_table.type_reference(member) {
                TypeReferenceNode::Named { symbol, .. } => records.push(*symbol),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Constrained {
                    base_type: element_type,
                    ..
                } => members.push(*element_type),
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn normalized_schema_report_fingerprint(
    typed: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> u64 {
    typed_trees::identity::normalized_schema_report_fingerprint(typed, data)
}

fn declared_source_bits(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    primitive: PrimitiveType,
    byte_size: u64,
) -> u64 {
    if primitive == PrimitiveType::Bool {
        return 1;
    }
    let Some(range) = validation::scalar_representation_range(typed, type_reference) else {
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
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ReflectedFieldLayout> {
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((
            size,
            size,
            declared_source_bits(typed, type_reference, primitive, size),
            Some(primitive),
            "Scalar",
            validation::scalar_representation_range(typed, type_reference)
                .map(|range| (range.minimum, range.maximum)),
            None,
        ));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
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
        typed_trees::types::TypeReferenceNode::Named { symbol, name } => {
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
    symbol: symbols::SymbolHandle,
    name: &str,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Option<(u64, u64)> {
    let data = typed.data_definitions().iter().find(|data| {
        if symbol.is_valid() {
            data.symbol == symbol
        } else {
            data.name.as_str() == name
        }
    })?;
    if let Some(layout) = typed.plan_laid_layout(data.symbol) {
        return match &layout.realizations {
            typed_trees::PlanLaidRealizations::Invariant(geometry) => Some((
                geometry.validated_layout.size?,
                geometry.validated_layout.align,
            )),
            typed_trees::PlanLaidRealizations::TargetFamily(_) => None,
        };
    }
    if data.quotient.is_some()
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
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
        let Some(field) = member.as_field() else {
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
    type_reference: typed_trees::types::TypeReferenceHandle,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Option<(u64, u64)> {
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let size = primitive_byte_size(primitive)?;
        return Some((size, size));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element_size, element_align) =
                reflected_nested_member_layout(typed, *element_type, visiting)?;
            Some((
                element_size.checked_mul(u64::try_from(*length).ok()?)?,
                element_align,
            ))
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, name } => {
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
