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
#[allow(unused_imports)]
pub use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::PrimitiveType;

use crate::BuildTimeAdmissionPlan;

mod owned_value_encoding;
mod plan_validation;
mod schema_value;

use owned_value_encoding::{encode_typed_owned_value, exact_struct_fields};
pub(crate) use plan_validation::validate_plan;
pub(crate) use schema_value::build_schema_value;
use schema_value::field_key;

const SCHEMA_FIELD_CAPACITY: usize = 32;

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
