//! Decoding, validation, and normalization of evaluated layout plans.

use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use psi_typed_trees::types::PrimitiveType;

use super::{BuildTimeValue, SchemaFieldInfo};

const PLAN_ENTRY_CAPACITY: usize = 64;

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
