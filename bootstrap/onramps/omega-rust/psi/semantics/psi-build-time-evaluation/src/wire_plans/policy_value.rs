use psi_checked_interpreter::BuildTimeValue;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::wire::WirePlacement;

use super::{FieldShape, WIRE_GRAMMAR_POLICY};

/// Evaluate `CompactBinary::plan` (contract-gated, build-time) against the
/// schema's facts and extract the authored placements, TAG-SORTED.
pub(super) fn evaluate_wire_policy(
    typed: &TypedTrees,
    admission: &crate::BuildTimeAdmissionPlan,
    schema_name: &str,
    fields: &[(u64, FieldShape)],
    custody: crate::BuildTimeInvocationCustody,
) -> Result<Vec<WirePlacement>, String> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == WIRE_GRAMMAR_POLICY)
        .expect("caller checked the policy exists");
    admission.require_common_floor_for_invocation(typed, machine, custody)?;

    let schema_value = build_wire_schema_value(fields);
    let plan = psi_checked_interpreter::evaluate_build_time_machine(
        typed,
        WIRE_GRAMMAR_POLICY,
        vec![schema_value],
    )
    .map_err(|reason| {
        format!(
            "build-time evaluation of `{WIRE_GRAMMAR_POLICY}` failed for `{schema_name}`: \
             {reason}"
        )
    })?;

    extract_wire_placements(&plan, fields.len(), schema_name)
}

/// Materialize the schema's facts as the std `Schema` value: field
/// (size, align, number, KIND) in declaration order, padded to 32.
fn build_wire_schema_value(fields: &[(u64, FieldShape)]) -> BuildTimeValue {
    let mut cells = Vec::with_capacity(32);
    for index in 0..32usize {
        let (number, shape) = fields
            .get(index)
            .copied()
            .unwrap_or((0, FieldShape::Scalar { byte_size: 0 }));
        let size = match shape {
            FieldShape::Scalar { byte_size } => byte_size,
            _ => 0,
        };
        cells.push(BuildTimeValue::Struct {
            type_name: "SchemaField".to_owned(),
            fields: vec![
                ("size".to_owned(), BuildTimeValue::Int(size as i64)),
                ("align".to_owned(), BuildTimeValue::Int(size.max(1) as i64)),
                // BuildTimeValue stores integer bits in i64; the policy's
                // typed u64 view reinterprets this losslessly.
                ("number".to_owned(), BuildTimeValue::Int(number as i64)),
                (
                    "kind".to_owned(),
                    BuildTimeValue::Case {
                        variant: shape.kind_case().to_owned(),
                        payload: Vec::new(),
                    },
                ),
            ],
        });
    }
    BuildTimeValue::Struct {
        type_name: "Schema".to_owned(),
        fields: vec![
            ("fields".to_owned(), BuildTimeValue::Array(cells)),
            (
                "field_count".to_owned(),
                BuildTimeValue::Int(fields.len() as i64),
            ),
        ],
    }
}

/// Extract the authored placements from the returned `Plan`: walk
/// `fields[0..entry_count]` FieldPlan cases (`Varint(tag)` /
/// `LengthPrefixed(tag)`; a layout placement like `At` is not a wire
/// placement), then sort by tag (the emission order).
fn extract_wire_placements(
    plan: &BuildTimeValue,
    field_count: usize,
    schema_name: &str,
) -> Result<Vec<WirePlacement>, String> {
    let fail = |reason: String| {
        format!("`{WIRE_GRAMMAR_POLICY}` produced an invalid plan for `{schema_name}`: {reason}")
    };
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
    let BuildTimeValue::Int(entry_count) = field("entry_count")? else {
        return Err(fail("entry_count is not an integer".to_owned()));
    };
    let entry_count = *entry_count as u64;
    if entry_count != field_count as u64 {
        return Err(fail(format!(
            "entry_count is {entry_count}, but the schema has {field_count} fields"
        )));
    }
    let BuildTimeValue::Array(cells) = field("fields")? else {
        return Err(fail(
            "plan `fields` is not an array of FieldPlan cases".to_owned(),
        ));
    };
    fn case_name(variant: &str) -> &str {
        variant.rsplit("::").next().unwrap_or(variant)
    }
    let tag_of = |payload: &[(String, BuildTimeValue)]| -> Result<u64, String> {
        match payload.iter().find(|(name, _)| name == "tag") {
            Some((_, BuildTimeValue::Int(tag))) => Ok(*tag as u64),
            other => Err(fail(format!("placement carries no integer tag: {other:?}"))),
        }
    };

    let mut placements = Vec::with_capacity(field_count);
    for index in 0..field_count {
        match cells.get(index) {
            Some(BuildTimeValue::Case { variant, payload }) => match case_name(variant) {
                "Varint" => placements.push(WirePlacement::Varint {
                    tag: tag_of(payload)?,
                }),
                "LengthPrefixed" => placements.push(WirePlacement::LengthPrefixed {
                    tag: tag_of(payload)?,
                }),
                other => {
                    return Err(fail(format!(
                        "placement {index} is `{other}`: a wire plan places fields with \
                         `Varint(tag)` or `LengthPrefixed(tag)`"
                    )));
                }
            },
            other => {
                return Err(fail(format!(
                    "placement entry {index} is missing or not a FieldPlan case: {other:?}"
                )));
            }
        }
    }
    placements.sort_by_key(|placement| placement.tag());
    Ok(placements)
}
