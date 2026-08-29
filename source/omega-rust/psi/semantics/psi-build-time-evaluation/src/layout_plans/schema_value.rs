//! Construction of the compiler-owned build-time `Schema` ABI value.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataMember, DataShapeKind};

use super::{BuildTimeValue, SCHEMA_FIELD_CAPACITY, SchemaFieldInfo, reflected_field_layout};

pub(crate) fn local_schema_field_discriminator(schema: &str, field: &str) -> u64 {
    // Stable FNV-1a. This value is only a compiler-policy-local lookup key:
    // every reflected scope rejects collisions before exposing it, and exact
    // schema structure supplies durable identity. Zero remains the unused-tail
    // sentinel.
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
    reject_local_discriminator_collisions(
        schema_data,
        "case",
        variants.iter().map(|variant| {
            (
                variant.name.to_string(),
                local_schema_field_discriminator(schema_data, variant.name.as_str()),
            )
        }),
    )?;
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
        reject_local_discriminator_collisions(
            schema_data,
            "payload field",
            payload.iter().map(|field| {
                let qualified = format!("{}::{}", variant.name, field.name);
                let key = local_schema_field_discriminator(schema_data, qualified.as_str());
                (field.name.to_string(), key)
            }),
        )?;
        let payload_fields = (0..SCHEMA_FIELD_CAPACITY)
            .map(|payload_index| {
                let info = payload.get(payload_index).map(|field| {
                    let reflected = reflected_field_layout(typed, field.type_reference);
                    let (size, align, source_bits, primitive, kind, declared_range, repeated) =
                        reflected.unwrap_or((0, 1, 0, None, "Nested", None, None));
                    SchemaFieldInfo {
                        name: field.name.to_string(),
                        identity: field.identity,
                        key: local_schema_field_discriminator(
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
                    BuildTimeValue::Int(local_schema_field_discriminator(
                        schema_data,
                        variant.name.as_str(),
                    ) as i64),
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

fn reject_local_discriminator_collisions(
    schema: &str,
    scope: &str,
    entries: impl IntoIterator<Item = (String, u64)>,
) -> Result<(), String> {
    let mut seen = std::collections::BTreeMap::new();
    for (name, discriminator) in entries {
        if let Some(prior) = seen.insert(discriminator, name.clone()) {
            return Err(format!(
                "schema data `{schema}` has a compiler-local {scope} discriminator collision between `{prior}` and `{name}`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::reject_local_discriminator_collisions;

    #[test]
    fn local_schema_discriminator_collisions_fail_closed() {
        let error = reject_local_discriminator_collisions(
            "Packet",
            "field",
            [("header".to_owned(), 7), ("payload".to_owned(), 7)],
        )
        .expect_err("compact-equal schema discriminators must reject");
        assert!(error.contains("`header`") && error.contains("`payload`"));
    }
}
