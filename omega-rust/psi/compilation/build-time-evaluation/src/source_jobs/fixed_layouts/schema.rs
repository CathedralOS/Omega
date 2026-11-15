//! Construct the policy's ordinary Schema value from its actual field roster.
//! Inactive array tails are valid records with alignment one; they do not name
//! a field that a returned layout may place.

use crate::BuildTimeValue as Value;
use typed_trees::{PlanLaidFieldLayoutSource, PlanLaidSchemaField, PlanLaidSchemaFieldKind};

const FIELD_CAPACITY: usize = 32;
const CASE_CAPACITY: usize = 32;

fn record(name: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Struct {
        type_name: name.into(),
        fields: fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    }
}

fn case(name: &str) -> Value {
    Value::Case {
        variant: name.into(),
        payload: Vec::new(),
    }
}

fn count(value: u64) -> Value {
    // The declared formal supplies signedness. BuildTimeValue carries the
    // same 64 bits for u64 and i64, including identities above i64::MAX.
    Value::Int(value as i64)
}

fn optional_identity(identity: Option<u64>) -> Value {
    match identity {
        None => case("None"),
        Some(identity) => Value::Case {
            variant: "Some".into(),
            payload: vec![("value".into(), count(identity))],
        },
    }
}

fn field(field: Option<&PlanLaidSchemaField>) -> Value {
    let key = field.map_or(0, |field| field.key);
    let size = field.map_or(0, |field| field.byte_size);
    let align = field.map_or(1, |field| field.byte_alignment);
    let identity = field.and_then(|field| field.identity);
    let number = identity.map_or(-1, |identity| identity as i64);
    let kind = match field.map(|field| field.kind) {
        None | Some(PlanLaidSchemaFieldKind::Scalar) => "Scalar",
        Some(PlanLaidSchemaFieldKind::Nested) => "Nested",
        Some(PlanLaidSchemaFieldKind::Repeated) => "Repeated",
    };
    let source = match field.map(|field| field.layout_source) {
        None | Some(PlanLaidFieldLayoutSource::Default) => "Default",
        Some(PlanLaidFieldLayoutSource::Scalar) => "Scalar",
        Some(PlanLaidFieldLayoutSource::Explicit) => "Explicit",
    };
    record(
        "SchemaField",
        [
            ("key", count(key)),
            ("size", count(size)),
            ("align", count(align)),
            ("identity", optional_identity(identity)),
            ("number", Value::Int(number)),
            ("kind", case(kind)),
            ("layout_source", case(source)),
        ],
    )
}

fn vacant_case(placeholder: &Value) -> Value {
    record(
        "SchemaCase",
        [
            ("key", count(0)),
            ("identity", optional_identity(None)),
            (
                "payload_fields",
                Value::Array(vec![placeholder.clone(); FIELD_CAPACITY]),
            ),
            ("payload_field_count", count(0)),
            (
                "retired_payload_identities",
                Value::Array(vec![count(0); FIELD_CAPACITY]),
            ),
            ("retired_payload_identity_count", count(0)),
        ],
    )
}

/// The core Schema has fixed array capacities. Preserve every actual field
/// and retirement, or refuse before calling the policy; truncation could turn
/// an incomplete plan into an apparently complete one.
pub(super) fn record_argument(
    fields: &[PlanLaidSchemaField],
    retired: &[u64],
) -> Result<Value, String> {
    if fields.len() > FIELD_CAPACITY || retired.len() > FIELD_CAPACITY {
        return Err("record exceeds the core layout Schema's complete field capacity".into());
    }
    let placeholder = field(None);
    let mut values = fields
        .iter()
        .map(|entry| field(Some(entry)))
        .collect::<Vec<_>>();
    values.resize(FIELD_CAPACITY, placeholder.clone());
    let mut retirements = retired.iter().copied().map(count).collect::<Vec<_>>();
    retirements.resize(FIELD_CAPACITY, count(0));
    let fields = u64::try_from(fields.len()).map_err(|_| "layout field count exceeds u64")?;
    let retired =
        u64::try_from(retired.len()).map_err(|_| "layout retirement count exceeds u64")?;
    Ok(record(
        "Schema",
        [
            ("fields", Value::Array(values)),
            ("field_count", count(fields)),
            ("retired_field_identities", Value::Array(retirements)),
            ("retired_field_identity_count", count(retired)),
            (
                "cases",
                Value::Array(vec![vacant_case(&placeholder); CASE_CAPACITY]),
            ),
            ("case_count", count(0)),
            (
                "retired_case_identities",
                Value::Array(vec![count(0); CASE_CAPACITY]),
            ),
            ("retired_case_identity_count", count(0)),
        ],
    ))
}
