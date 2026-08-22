//! Programmable-layout evaluation and validation.
//!
//! The compiler materializes `Schema`, invokes a build-time-admissible policy
//! machine at build time, and validates the returned `Plan` before any consumer
//! trusts it. Plans are keyed by compiler-issued field identities rather than
//! array position, so policies may reorder entries or fragment one logical
//! field.

pub use psi_checked_interpreter::BuildTimeValue;
use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, MaterializationDiagnostic,
    materialize_aggregate_layout_into,
};
pub use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::PrimitiveType;

use crate::BuildTimeAdmissionPlan;

mod owned_value_encoding;

use owned_value_encoding::{encode_typed_owned_value, exact_struct_fields};

const SCHEMA_FIELD_CAPACITY: usize = 32;
const PLAN_ENTRY_CAPACITY: usize = 64;

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

pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<LayoutPlanReport, String> {
    let (schema_fields, schema_identity) = schema_fields(typed, schema_data)?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    BuildTimeAdmissionPlan::infer(typed).require_common_floor(typed, machine)?;

    let plan = psi_checked_interpreter::evaluate_build_time_machine(
        typed,
        policy_machine,
        vec![schema_value],
    )
    .map_err(|reason| format!("build-time evaluation of `{policy_machine}` failed: {reason}"))?;

    validate_plan(&plan, &schema_fields, schema_identity, policy_machine)
}

/// Materializes one compiler-checked, source-owned record through a normalized
/// layout. Psi supplies the typed semantic value and derives every native field
/// extent; the Omega realization seam supplies byte order. Source code never
/// supplies physical field bytes or offsets.
pub fn materialize_typed_owned_layout_into(
    typed: &TypedTrees,
    schema_data: &str,
    layout: &LayoutPlanReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "no typed data definition named `{schema_data}` exists"
            ))
        })?;
    if layout.schema_identity != normalized_schema_identity(typed, data) {
        return Err(MaterializationDiagnostic(format!(
            "layout schema identity does not match typed data `{schema_data}`"
        )));
    }
    let (schema_fields, _) =
        schema_fields(typed, schema_data).map_err(MaterializationDiagnostic)?;
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value for `{schema_data}` is not a record"
        )));
    };
    if type_name != schema_data {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value `{type_name}` does not match schema `{schema_data}`"
        )));
    }
    let supplied = exact_struct_fields(schema_data, fields)?;
    let members = typed.data_members(data);
    if members
        .iter()
        .any(|member| matches!(member, psi_typed_trees::data::DataMember::Variant(_)))
    {
        return Err(MaterializationDiagnostic(format!(
            "typed owned materialization does not yet admit sum cases in `{schema_data}`"
        )));
    }
    let physical_fields = members
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value `{schema_data}` has {} fields, expected {}",
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
                "typed owned value `{schema_data}` has no field `{}`",
                field.name
            )));
        }
    }

    let mut schemas = Vec::with_capacity(schema_fields.len());
    let mut values = Vec::with_capacity(schema_fields.len());
    for field in &physical_fields {
        if schema_fields
            .iter()
            .any(|reflected| reflected.name == field.name.as_str())
        {
            continue;
        }
        let field_value = supplied.get(field.name.as_str()).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "typed owned value `{schema_data}` has no field `{}`",
                field.name
            ))
        })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut Vec::new(),
        )?;
        if !bytes.is_empty() {
            return Err(MaterializationDiagnostic(format!(
                "typed field `{}` was omitted from layout despite deriving {} physical bytes",
                field.name,
                bytes.len()
            )));
        }
    }
    for reflected in &schema_fields {
        let field = physical_fields
            .iter()
            .find(|field| field.name.as_str() == reflected.name)
            .expect("schema reflection retained the same physical fields");
        let field_value = supplied.get(reflected.name.as_str()).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "typed owned value `{schema_data}` has no field `{}`",
                reflected.name
            ))
        })?;
        let bytes = encode_typed_owned_value(
            typed,
            field.type_reference,
            field_value,
            byte_order,
            &mut Vec::new(),
        )?;
        let byte_size = u64::try_from(bytes.len()).map_err(|_| {
            MaterializationDiagnostic(format!(
                "typed field `{}` extent cannot be represented as u64",
                reflected.name
            ))
        })?;
        if byte_size != reflected.size {
            return Err(MaterializationDiagnostic(format!(
                "typed field `{}` encoded to {byte_size} bytes, expected {}",
                reflected.name, reflected.size
            )));
        }
        schemas.push(match (reflected.repeated, reflected.identity) {
            (Some(repeated), Some(identity)) => AggregateFieldSchema::new_repeated_numbered(
                &reflected.name,
                identity,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (Some(repeated), None) => AggregateFieldSchema::new_repeated(
                &reflected.name,
                repeated.element_size,
                repeated.element_align,
                repeated.element_count,
            )?,
            (None, Some(identity)) => {
                AggregateFieldSchema::new_numbered(&reflected.name, identity, reflected.size)?
            }
            (None, None) => AggregateFieldSchema::new(&reflected.name, reflected.size)?,
        });
        values.push(AggregateFieldValue::new(&reflected.name, bytes)?);
    }
    materialize_aggregate_layout_into(layout, &schemas, &values, destination)
}

/// Evaluates an effect-free, zero-argument source machine to obtain an owned
/// typed value, then materializes it through the selected layout. The compiler
/// owns both the evaluation boundary and type-directed byte derivation.
pub fn evaluate_and_materialize_typed_owned_layout_into(
    typed: &TypedTrees,
    value_machine: &str,
    schema_data: &str,
    layout: &LayoutPlanReport,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == value_machine)
        .ok_or_else(|| {
            MaterializationDiagnostic(format!("no machine named `{value_machine}` exists"))
        })?;
    let states = typed.machine_states(machine);
    let [entry] = states else {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value machine `{value_machine}` must have one state"
        )));
    };
    if !typed.state_parameters(entry).is_empty() {
        return Err(MaterializationDiagnostic(format!(
            "typed owned value machine `{value_machine}` must take no arguments"
        )));
    }
    BuildTimeAdmissionPlan::infer(typed)
        .require_common_floor(typed, machine)
        .map_err(MaterializationDiagnostic)?;
    let value = psi_checked_interpreter::evaluate_build_time_machine(typed, value_machine, vec![])
        .map_err(|reason| {
            MaterializationDiagnostic(format!(
                "build-time evaluation of typed owned value `{value_machine}` failed: {reason}"
            ))
        })?;
    materialize_typed_owned_layout_into(typed, schema_data, layout, &value, byte_order, destination)
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
    use psi_typed_trees::data::DataMember;

    fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    fn member_name(hash: &mut u64, identity: Option<u64>, name: &str, position: usize) {
        match identity {
            Some(identity) => {
                byte(hash, 1);
                uint(hash, identity);
            }
            None => {
                byte(hash, 0);
                uint(hash, position as u64);
                text(hash, name);
            }
        }
    }

    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, b"omega.schema.v2");
    let members = typed.data_members(data);
    let mut fields = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let mut cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if fields.iter().all(|field| field.identity.is_some()) {
        fields.sort_by_key(|field| field.identity);
    }
    if cases.iter().all(|case| case.identity.is_some()) {
        cases.sort_by_key(|case| case.identity);
    }
    uint(&mut hash, fields.len() as u64);
    for (position, field) in fields.iter().enumerate() {
        member_name(&mut hash, field.identity, field.name.as_str(), position);
        byte(
            &mut hash,
            match field.relevance {
                psi_language_core::BindingRelevance::Relevant => 0,
                psi_language_core::BindingRelevance::Erased => 1,
            },
        );
        text(
            &mut hash,
            typed.display_type_reference(field.type_reference).as_str(),
        );
    }
    uint(&mut hash, cases.len() as u64);
    for (position, case) in cases.iter().enumerate() {
        member_name(&mut hash, case.identity, case.name.as_str(), position);
        let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
        if payload.iter().all(|field| field.identity.is_some()) {
            payload.sort_by_key(|field| field.identity);
        }
        uint(&mut hash, payload.len() as u64);
        for (payload_position, field) in payload.iter().enumerate() {
            member_name(
                &mut hash,
                field.identity,
                field.name.as_str(),
                payload_position,
            );
            byte(
                &mut hash,
                match field.relevance {
                    psi_language_core::BindingRelevance::Relevant => 0,
                    psi_language_core::BindingRelevance::Erased => 1,
                },
            );
            text(
                &mut hash,
                typed.display_type_reference(field.type_reference).as_str(),
            );
        }
        let mut retired = case.retired_payload_identities.clone();
        retired.sort_unstable();
        uint(&mut hash, retired.len() as u64);
        for identity in retired {
            uint(&mut hash, identity);
        }
    }
    let mut retired = data.retired_identities.clone();
    retired.sort_unstable();
    uint(&mut hash, retired.len() as u64);
    for identity in retired {
        uint(&mut hash, identity);
    }
    if hash == 0 { 1 } else { hash }
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

fn primitive_byte_size(primitive: PrimitiveType) -> Option<u64> {
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

fn reflected_field_layout(
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

fn reflected_nested_member_layout(
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

fn checked_align_up(value: u64, align: u64) -> Option<u64> {
    if align == 0 {
        return None;
    }
    value
        .checked_add(align - 1)
        .map(|value| value / align * align)
}

fn field_key(schema: &str, field: &str) -> u64 {
    // Stable FNV-1a. Zero remains the unused-tail sentinel.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in schema.bytes().chain([b':', b':']).chain(field.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    if hash == 0 { 1 } else { hash }
}

fn optional_identity(identity: Option<u64>) -> BuildTimeValue {
    match identity {
        Some(identity) => BuildTimeValue::Case {
            variant: "Some".to_owned(),
            payload: vec![("value".to_owned(), BuildTimeValue::Int(identity as i64))],
        },
        None => BuildTimeValue::Case {
            variant: "None".to_owned(),
            payload: Vec::new(),
        },
    }
}

fn padded_identities(identities: &[u64]) -> Vec<BuildTimeValue> {
    (0..SCHEMA_FIELD_CAPACITY)
        .map(|index| BuildTimeValue::Int(identities.get(index).copied().unwrap_or_default() as i64))
        .collect()
}

fn build_schema_field_value(field: Option<&SchemaFieldInfo>) -> BuildTimeValue {
    let (key, size, align, identity, kind) = field
        .map(|field| {
            (
                field.key,
                field.size,
                field.align,
                field.identity,
                field.kind,
            )
        })
        .unwrap_or((0, 0, 1, None, "Scalar"));
    BuildTimeValue::Struct {
        type_name: "SchemaField".to_owned(),
        fields: vec![
            ("key".to_owned(), BuildTimeValue::Int(key as i64)),
            ("size".to_owned(), BuildTimeValue::Int(size as i64)),
            ("align".to_owned(), BuildTimeValue::Int(align as i64)),
            ("identity".to_owned(), optional_identity(identity)),
            (
                "number".to_owned(),
                BuildTimeValue::Int(identity.map(|identity| identity as i64).unwrap_or(-1)),
            ),
            (
                "kind".to_owned(),
                BuildTimeValue::Case {
                    variant: kind.to_owned(),
                    payload: Vec::new(),
                },
            ),
        ],
    }
}

pub(crate) fn build_schema_value(
    typed: &TypedTrees,
    schema_data: &str,
    schema_fields: &[SchemaFieldInfo],
) -> Result<BuildTimeValue, String> {
    use psi_typed_trees::data::{DataMember, DataShapeKind};

    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;
    let mut fields = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
    for index in 0..SCHEMA_FIELD_CAPACITY {
        fields.push(build_schema_field_value(schema_fields.get(index)));
    }
    let variants = typed
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .collect::<Vec<_>>();
    if variants.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} cases; reflected Schema supports at most {}",
            variants.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    let mut cases = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
    for index in 0..SCHEMA_FIELD_CAPACITY {
        let Some(variant) = variants.get(index).copied() else {
            cases.push(BuildTimeValue::Struct {
                type_name: "SchemaCase".to_owned(),
                fields: vec![
                    ("key".to_owned(), BuildTimeValue::Int(0)),
                    ("identity".to_owned(), optional_identity(None)),
                    (
                        "payload_fields".to_owned(),
                        BuildTimeValue::Array(
                            (0..SCHEMA_FIELD_CAPACITY)
                                .map(|_| build_schema_field_value(None))
                                .collect(),
                        ),
                    ),
                    ("payload_field_count".to_owned(), BuildTimeValue::Int(0)),
                    (
                        "retired_payload_identities".to_owned(),
                        BuildTimeValue::Array(padded_identities(&[])),
                    ),
                    (
                        "retired_payload_identity_count".to_owned(),
                        BuildTimeValue::Int(0),
                    ),
                ],
            });
            continue;
        };
        let payload = typed.data_payload_fields(variant);
        if payload.len() > SCHEMA_FIELD_CAPACITY
            || variant.retired_payload_identities.len() > SCHEMA_FIELD_CAPACITY
        {
            return Err(format!(
                "schema data `{schema_data}` case `{}` exceeds the reflected Schema capacity of {} payload fields or tombstones",
                variant.name, SCHEMA_FIELD_CAPACITY
            ));
        }
        let payload_fields = (0..SCHEMA_FIELD_CAPACITY)
            .map(|payload_index| {
                let info = payload.get(payload_index).map(|field| {
                    let reflected = reflected_field_layout(typed, field.type_reference);
                    let (size, align, source_bits, primitive, kind, declared_range, repeated) =
                        reflected.unwrap_or((0, 1, 0, None, "Nested", None, None));
                    SchemaFieldInfo {
                        name: field.name.to_string(),
                        identity: field.identity,
                        key: field_key(
                            schema_data,
                            format!("{}::{}", variant.name, field.name).as_str(),
                        ),
                        size,
                        align,
                        source_bits,
                        primitive,
                        kind,
                        declared_range,
                        repeated,
                    }
                });
                build_schema_field_value(info.as_ref())
            })
            .collect();
        cases.push(BuildTimeValue::Struct {
            type_name: "SchemaCase".to_owned(),
            fields: vec![
                (
                    "key".to_owned(),
                    BuildTimeValue::Int(field_key(schema_data, variant.name.as_str()) as i64),
                ),
                ("identity".to_owned(), optional_identity(variant.identity)),
                (
                    "payload_fields".to_owned(),
                    BuildTimeValue::Array(payload_fields),
                ),
                (
                    "payload_field_count".to_owned(),
                    BuildTimeValue::Int(payload.len() as i64),
                ),
                (
                    "retired_payload_identities".to_owned(),
                    BuildTimeValue::Array(padded_identities(&variant.retired_payload_identities)),
                ),
                (
                    "retired_payload_identity_count".to_owned(),
                    BuildTimeValue::Int(variant.retired_payload_identities.len() as i64),
                ),
            ],
        });
    }
    let shape =
        psi_typed_trees::data::DataDefinition::shape_kind_from_members(typed.data_members(data));
    let (retired_fields, retired_cases) = match shape {
        DataShapeKind::Record => (data.retired_identities.as_slice(), &[][..]),
        DataShapeKind::Enum => (&[][..], data.retired_identities.as_slice()),
        DataShapeKind::Empty | DataShapeKind::Mixed => (&[][..], &[][..]),
    };
    Ok(BuildTimeValue::Struct {
        type_name: "Schema".to_owned(),
        fields: vec![
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            (
                "field_count".to_owned(),
                BuildTimeValue::Int(schema_fields.len() as i64),
            ),
            (
                "retired_field_identities".to_owned(),
                BuildTimeValue::Array(padded_identities(retired_fields)),
            ),
            (
                "retired_field_identity_count".to_owned(),
                BuildTimeValue::Int(retired_fields.len() as i64),
            ),
            ("cases".to_owned(), BuildTimeValue::Array(cases)),
            (
                "case_count".to_owned(),
                BuildTimeValue::Int(variants.len() as i64),
            ),
            (
                "retired_case_identities".to_owned(),
                BuildTimeValue::Array(padded_identities(retired_cases)),
            ),
            (
                "retired_case_identity_count".to_owned(),
                BuildTimeValue::Int(retired_cases.len() as i64),
            ),
        ],
    })
}

pub(crate) fn validate_plan(
    plan: &BuildTimeValue,
    schema_fields: &[SchemaFieldInfo],
    schema_identity: u64,
    policy_machine: &str,
) -> Result<LayoutPlanReport, String> {
    let fail =
        |reason: String| format!("policy `{policy_machine}` produced an invalid plan: {reason}");
    let BuildTimeValue::Struct { fields, .. } = plan else {
        return Err(fail(format!("expected a Plan struct, got {plan:?}")));
    };
    let field = |name: &str| -> Result<&BuildTimeValue, String> {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
            .ok_or_else(|| fail(format!("the plan carries no `{name}` field")))
    };
    // Build-time integer values preserve the declared scalar's bits in an
    // `i64` carrier. Decode u64 policy fields by bits, not by sign.
    let uint_field = |name: &str| -> Result<u64, String> {
        match field(name)? {
            BuildTimeValue::Int(value) => Ok(*value as u64),
            other => Err(fail(format!(
                "plan field `{name}` is not an integer: {other:?}"
            ))),
        }
    };

    let entry_count = uint_field("entry_count")?;
    if entry_count > PLAN_ENTRY_CAPACITY as u64 {
        return Err(fail(format!(
            "entry_count {entry_count} is outside 0..={PLAN_ENTRY_CAPACITY}"
        )));
    }
    let entry_count = entry_count as usize;
    let size_is_dynamic = match field("size_is_dynamic")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(fail(format!("size_is_dynamic is not a bool: {other:?}"))),
    };
    let size_fixed = uint_field("size_fixed")?;
    let align = uint_field("align")?;
    if align == 0 || !align.is_power_of_two() {
        return Err(fail(format!(
            "alignment {align} is not a positive power of two"
        )));
    }
    let BuildTimeValue::Array(entry_cells) = field("entries")? else {
        return Err(fail(
            "plan `entries` is not an array of FieldEntry values".to_owned(),
        ));
    };
    if entry_count > entry_cells.len() {
        return Err(fail(format!(
            "entry_count is {entry_count}, but the plan carries only {} entries",
            entry_cells.len()
        )));
    }

    let case_name = |variant: &str| variant.rsplit("::").next().unwrap_or(variant).to_owned();
    let lookup = |key: u64| schema_fields.iter().position(|field| field.key == key);
    let payload_uint = |payload: &[(String, BuildTimeValue)], name: &str| -> Result<u64, String> {
        match payload.iter().find(|(field, _)| field == name) {
            Some((_, BuildTimeValue::Int(value))) => Ok(*value as u64),
            other => Err(fail(format!(
                "placement carries no integer `{name}`: {other:?}"
            ))),
        }
    };

    // A repeated `At` changes the extent represented by each entry, so count
    // those entries before validating destination intervals. Only a reflected
    // outer fixed array may use more than one.
    let mut at_counts = vec![0usize; schema_fields.len()];
    for entry_index in 0..entry_count {
        let Some(BuildTimeValue::Struct { fields, .. }) = entry_cells.get(entry_index) else {
            return Err(fail(format!(
                "entry {entry_index} is missing or not a FieldEntry"
            )));
        };
        let key = match fields.iter().find(|(name, _)| name == "key") {
            Some((_, BuildTimeValue::Int(key))) => *key as u64,
            other => {
                return Err(fail(format!(
                    "entry {entry_index} has no integer key: {other:?}"
                )));
            }
        };
        let Some(field_index) = lookup(key) else {
            return Err(fail(format!(
                "entry {entry_index} refers to unknown field key {key}"
            )));
        };
        let placement = fields
            .iter()
            .find(|(name, _)| name == "placement")
            .map(|(_, value)| value)
            .ok_or_else(|| fail(format!("entry {entry_index} carries no `placement`")))?;
        if let BuildTimeValue::Case { variant, .. } = placement
            && case_name(variant) == "At"
        {
            at_counts[field_index] += 1;
        }
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut source_spans: Vec<Vec<(u64, u64)>> = vec![Vec::new(); schema_fields.len()];
    let mut repeated_at_offsets: Vec<Vec<u64>> = vec![Vec::new(); schema_fields.len()];
    let mut destination_spans: Vec<(u64, u64, String)> = Vec::new();
    let mut offsets_by_field = vec![None; schema_fields.len()];
    let mut kinds_by_field: Vec<Option<&'static str>> = vec![None; schema_fields.len()];

    for entry_index in 0..entry_count {
        let Some(BuildTimeValue::Struct { fields, .. }) = entry_cells.get(entry_index) else {
            return Err(fail(format!(
                "entry {entry_index} is missing or not a FieldEntry"
            )));
        };
        let key = match fields.iter().find(|(name, _)| name == "key") {
            Some((_, BuildTimeValue::Int(key))) => *key as u64,
            other => {
                return Err(fail(format!(
                    "entry {entry_index} has no integer key: {other:?}"
                )));
            }
        };
        let Some(field_index) = lookup(key) else {
            return Err(fail(format!(
                "entry {entry_index} refers to unknown field key {key}"
            )));
        };
        let schema_field = &schema_fields[field_index];
        let placement = fields
            .iter()
            .find(|(name, _)| name == "placement")
            .map(|(_, value)| value)
            .ok_or_else(|| fail(format!("entry {entry_index} carries no `placement`")))?;
        let BuildTimeValue::Case { variant, payload } = placement else {
            return Err(fail(format!(
                "entry {entry_index} placement is not a FieldPlan case"
            )));
        };

        match case_name(variant).as_str() {
            "At" => {
                let tiled = at_counts[field_index] > 1;
                let placement_size = if tiled {
                    let Some(repeated) = schema_field.repeated else {
                        return Err(fail(format!(
                            "field `{}` has more than one `At` placement but is not an outer fixed array",
                            schema_field.name
                        )));
                    };
                    if at_counts[field_index] as u64 != repeated.element_count {
                        return Err(fail(format!(
                            "repeated field `{}` has {} `At` placements, expected one for each of its {} elements",
                            schema_field.name, at_counts[field_index], repeated.element_count
                        )));
                    }
                    if let Some(kind) = kinds_by_field[field_index]
                        && kind != "RepeatedAt"
                    {
                        return Err(fail(format!(
                            "field `{}` mixes repeated `At` with another placement",
                            schema_field.name
                        )));
                    }
                    kinds_by_field[field_index] = Some("RepeatedAt");
                    repeated.element_size
                } else {
                    if kinds_by_field[field_index].replace("At").is_some() {
                        return Err(fail(format!(
                            "field `{}` has more than one placement",
                            schema_field.name
                        )));
                    }
                    schema_field.size
                };
                let offset = payload_uint(payload, "offset")?;
                if offset % schema_field.align != 0 {
                    return Err(fail(format!(
                        "field `{}` at offset {offset} violates its alignment {}",
                        schema_field.name, schema_field.align
                    )));
                }
                let end = offset.checked_add(placement_size).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let start_bit = offset.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let end_bit = end.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                destination_spans.push((start_bit, end_bit, schema_field.name.clone()));
                if tiled {
                    repeated_at_offsets[field_index].push(offset);
                } else {
                    offsets_by_field[field_index] = Some(offset);
                }
                entries.push(LayoutFieldEntryReport {
                    field: schema_field.name.clone(),
                    member_identity: schema_field.identity,
                    placement: LayoutPlacementReport::At { offset },
                });
            }
            "IntegerAt" => {
                if kinds_by_field[field_index].replace("IntegerAt").is_some() {
                    return Err(fail(format!(
                        "field `{}` has more than one placement",
                        schema_field.name
                    )));
                }
                let offset = payload_uint(payload, "offset")?;
                let stored_width = payload_uint(payload, "stored_width")?;
                if stored_width == 0 || stored_width > 64 || !stored_width.is_multiple_of(8) {
                    return Err(fail(format!(
                        "field `{}` stored integer width {stored_width} is not a supported whole-byte width in 8..=64",
                        schema_field.name
                    )));
                }
                let Some(primitive) = schema_field.primitive else {
                    return Err(fail(format!(
                        "field `{}` uses `IntegerAt`, but aggregate fields support only `At` placement",
                        schema_field.name
                    )));
                };
                if !primitive.accepts_integer_literal() || primitive == PrimitiveType::Addr {
                    return Err(fail(format!(
                        "field `{}` uses `IntegerAt`, but its semantic type `{}` is not a fixed-width integer carrier",
                        schema_field.name,
                        primitive.name()
                    )));
                }
                let interpretation = match payload
                    .iter()
                    .find(|(field, _)| field == "interpretation")
                {
                    Some((_, BuildTimeValue::Case { variant, payload })) if payload.is_empty() => {
                        match case_name(variant).as_str() {
                            "Signed" => psi_layout_plans::IntegerInterpretation::Signed,
                            "Unsigned" => psi_layout_plans::IntegerInterpretation::Unsigned,
                            other => {
                                return Err(fail(format!(
                                    "field `{}` has unknown integer interpretation `{other}`",
                                    schema_field.name
                                )));
                            }
                        }
                    }
                    other => {
                        return Err(fail(format!(
                            "field `{}` carries no signed/unsigned integer interpretation: {other:?}",
                            schema_field.name
                        )));
                    }
                };
                validate_integer_decode_range(schema_field, stored_width, interpretation)?;
                let end = offset.checked_add(stored_width / 8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let start_bit = offset.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let end_bit = end.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                destination_spans.push((start_bit, end_bit, schema_field.name.clone()));
                entries.push(LayoutFieldEntryReport {
                    field: schema_field.name.clone(),
                    member_identity: schema_field.identity,
                    placement: LayoutPlacementReport::IntegerAt {
                        offset,
                        stored_width,
                        interpretation,
                    },
                });
            }
            "Bits" => {
                if schema_field.primitive.is_none() {
                    return Err(fail(format!(
                        "field `{}` uses `Bits`, but aggregate fields support only `At` placement",
                        schema_field.name
                    )));
                }
                if matches!(
                    kinds_by_field[field_index],
                    Some("At" | "RepeatedAt" | "IntegerAt")
                ) {
                    return Err(fail(format!(
                        "field `{}` mixes a whole-field and `Bits` placement",
                        schema_field.name
                    )));
                }
                kinds_by_field[field_index] = Some("Bits");
                let container = payload_uint(payload, "container")?;
                let container_width = payload_uint(payload, "container_width")?;
                let destination_lsb = payload_uint(payload, "destination_lsb")?;
                let source_lsb = payload_uint(payload, "source_lsb")?;
                let width = payload_uint(payload, "width")?;
                if container_width == 0 || width == 0 {
                    return Err(fail(format!(
                        "field `{}` has a non-positive or negative bit-fragment component",
                        schema_field.name
                    )));
                }
                let destination_end = destination_lsb
                    .checked_add(width)
                    .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                let source_end = source_lsb
                    .checked_add(width)
                    .ok_or_else(|| fail("source bit range overflows".to_owned()))?;
                if destination_end > container_width {
                    return Err(fail(format!(
                        "field `{}` fragment destination {}..{} exceeds its {container_width}-bit container",
                        schema_field.name, destination_lsb, destination_end
                    )));
                }
                if source_end > schema_field.source_bits {
                    return Err(fail(format!(
                        "field `{}` fragment source {}..{} exceeds its {}-bit value",
                        schema_field.name, source_lsb, source_end, schema_field.source_bits
                    )));
                }
                let absolute_start = container
                    .checked_mul(8)
                    .and_then(|base| base.checked_add(destination_lsb))
                    .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                let absolute_end = absolute_start
                    .checked_add(width)
                    .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                destination_spans.push((absolute_start, absolute_end, schema_field.name.clone()));
                source_spans[field_index].push((source_lsb, source_end));
                entries.push(LayoutFieldEntryReport {
                    field: schema_field.name.clone(),
                    member_identity: schema_field.identity,
                    placement: LayoutPlacementReport::Bits {
                        container,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    },
                });
            }
            other => {
                return Err(fail(format!(
                    "entry {entry_index} uses `{other}`; this fixed-layout validator supports `At` and `Bits`"
                )));
            }
        }
    }

    for (index, schema_field) in schema_fields.iter().enumerate() {
        match kinds_by_field[index] {
            None => {
                return Err(fail(format!(
                    "field `{}` has no placement",
                    schema_field.name
                )));
            }
            Some("At") => {}
            Some("RepeatedAt") => {
                let repeated = schema_field
                    .repeated
                    .expect("only a reflected outer fixed array enters repeated At");
                let offsets = &mut repeated_at_offsets[index];
                offsets.sort_unstable();
                let stride = offsets[1] - offsets[0];
                if stride < repeated.element_size
                    || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
                {
                    return Err(fail(format!(
                        "repeated field `{}` element placements do not have one nonoverlapping constant stride",
                        schema_field.name
                    )));
                }
            }
            Some("IntegerAt") => {}
            Some("Bits") => {
                let spans = &mut source_spans[index];
                spans.sort_unstable();
                let mut cursor = 0;
                for &(start, end) in spans.iter() {
                    if start != cursor {
                        return Err(fail(format!(
                            "field `{}` source fragments do not tile exactly: expected next bit {cursor}, found {start}",
                            schema_field.name
                        )));
                    }
                    cursor = end;
                }
                if cursor != schema_field.source_bits {
                    return Err(fail(format!(
                        "field `{}` source fragments end at bit {cursor}, expected {}",
                        schema_field.name, schema_field.source_bits
                    )));
                }
            }
            _ => unreachable!(),
        }
    }

    destination_spans.sort_by_key(|span| span.0);
    for pair in destination_spans.windows(2) {
        let (_, end_a, field_a) = &pair[0];
        let (start_b, _, field_b) = &pair[1];
        if start_b < end_a {
            return Err(fail(format!(
                "destination placements for fields `{field_a}` and `{field_b}` overlap"
            )));
        }
    }
    if !size_is_dynamic {
        if let Some((_, end, field_name)) = destination_spans.last() {
            let size_bits = size_fixed
                .checked_mul(8)
                .ok_or_else(|| fail(format!("fixed size {size_fixed} overflows in bits")))?;
            if *end > size_bits {
                return Err(fail(format!(
                    "field `{field_name}` ends at bit {end}, past the fixed size {size_bits} bits",
                )));
            }
        }
        if size_fixed % align != 0 {
            return Err(fail(format!(
                "fixed size {size_fixed} is not a multiple of the alignment {align}"
            )));
        }
    }

    // Authored entry order is presentation, not identity. Normalize by schema
    // declaration and then by logical source position so two policies that
    // describe the same geometry produce the same report.
    entries.sort_by_key(|entry| {
        let field_index = schema_fields
            .iter()
            .position(|field| field.name == entry.field)
            .unwrap_or(usize::MAX);
        let source_lsb = match &entry.placement {
            LayoutPlacementReport::At { offset }
                if schema_fields[field_index].repeated.is_some() =>
            {
                *offset
            }
            LayoutPlacementReport::At { .. } => 0,
            LayoutPlacementReport::IntegerAt { .. } => 0,
            LayoutPlacementReport::Bits { source_lsb, .. } => *source_lsb,
        };
        (field_index, source_lsb)
    });

    let offsets = offsets_by_field
        .iter()
        .all(Option::is_some)
        .then(|| offsets_by_field.into_iter().map(Option::unwrap).collect());

    Ok(LayoutPlanReport {
        schema_identity,
        entries,
        offsets,
        size: (!size_is_dynamic).then_some(size_fixed),
        align,
    })
}

fn validate_integer_decode_range(
    field: &SchemaFieldInfo,
    stored_width: u64,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> Result<(), String> {
    let primitive = field
        .primitive
        .ok_or_else(|| format!("field `{}` is not a scalar integer", field.name))?;
    let semantic_width = field.size * 8;
    let carrier_fits = match interpretation {
        psi_layout_plans::IntegerInterpretation::Signed => {
            primitive.is_signed_integer() && stored_width <= semantic_width
        }
        psi_layout_plans::IntegerInterpretation::Unsigned => {
            if primitive.is_signed_integer() {
                stored_width < semantic_width
            } else {
                stored_width <= semantic_width
            }
        }
    };
    if !carrier_fits {
        return Err(format!(
            "field `{}` cannot totally decode a {stored_width}-bit {} integer into `{}`",
            field.name,
            match interpretation {
                psi_layout_plans::IntegerInterpretation::Signed => "signed",
                psi_layout_plans::IntegerInterpretation::Unsigned => "unsigned",
            },
            primitive.name()
        ));
    }

    if let Some((minimum, maximum)) = field.declared_range {
        let (stored_minimum, stored_maximum) = match interpretation {
            psi_layout_plans::IntegerInterpretation::Signed => (
                -(1i128 << (stored_width - 1)),
                (1i128 << (stored_width - 1)) - 1,
            ),
            psi_layout_plans::IntegerInterpretation::Unsigned => (0, (1i128 << stored_width) - 1),
        };
        if stored_minimum < i128::from(minimum) || stored_maximum > i128::from(maximum) {
            return Err(format!(
                "field `{}` stored integer range {stored_minimum}..={stored_maximum} does not fit its declared semantic range {minimum}..={maximum}",
                field.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalized_schema_identity, schema_fields};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn semantic_schema_identity_distinguishes_common_and_payload_field_relevance() {
        let source = r#"
            data CommonRelevant { proof: i32; }
            data CommonErased { proof [erased]: i32; }
            data PayloadRelevant { case Certified(proof: i32); }
            data PayloadErased { case Certified(proof [erased]: i32); }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let identity = |name: &str| {
            let data = typed
                .data_definitions()
                .iter()
                .find(|data| data.name.as_str() == name)
                .expect("data definition");
            normalized_schema_identity(&typed, data)
        };

        assert_ne!(identity("CommonRelevant"), identity("CommonErased"));
        assert_ne!(identity("PayloadRelevant"), identity("PayloadErased"));
    }

    #[test]
    fn schema_reflection_rejects_quotients_directly_and_as_nested_records() {
        let source = r#"
            data Carrier { case Unit; }
            proposition same(left: Carrier, right: Carrier) = left == right;
            data Bucket = Carrier % same;
            data Envelope { bucket: Bucket; tag: u8; }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");

        let direct = schema_fields(&typed, "Bucket").expect_err("quotient schema must reject");
        assert!(direct.contains("schema reflection cannot observe quotient `Bucket`"));

        let nested = schema_fields(&typed, "Envelope")
            .expect_err("a nested quotient must not acquire a reflected layout");
        assert!(nested.contains("field `bucket` is neither a supported primitive"));
    }
}
