//! A policy returns ordinary data. Check every active entry against the exact
//! supplied field roster before any consumer can trust its offsets or extent.

use crate::BuildTimeValue as Value;
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use typed_trees::data::DataDefinition;
use typed_trees::{PlanLaidSchemaField, TypedTrees};

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    let Value::Struct { fields, .. } = value else {
        return Err(format!("layout result `{name}` requires a record"));
    };
    let mut found = fields.iter().filter(|(candidate, _)| candidate == name);
    let (_, value) = found
        .next()
        .ok_or_else(|| format!("layout result omits `{name}`"))?;
    if found.next().is_some() {
        return Err(format!("layout result repeats `{name}`"));
    }
    Ok(value)
}

fn count(value: &Value, name: &str) -> Result<u64, String> {
    let Value::Int(value) = value else {
        return Err(format!("layout `{name}` is not an integer"));
    };
    // These fields were returned under the actual core LayoutPlan's u64
    // declarations. The shared value boundary retains their unsigned bits
    // in an i64 container; interpreting its sign would lose the high half.
    Ok(*value as u64)
}

pub(super) fn record_plan(
    typed: &TypedTrees,
    data: &DataDefinition,
    schema: &[PlanLaidSchemaField],
    result: &Value,
) -> Result<LayoutPlanReport, String> {
    let entry_count = count(field(result, "entry_count")?, "entry_count")?;
    let Value::Array(cells) = field(result, "entries")? else {
        return Err("layout entries are not an array".into());
    };
    if cells.len() != 64 || entry_count > 64 {
        return Err("layout result does not retain its complete 64-entry carrier".into());
    }
    let Value::Bool(dynamic) = field(result, "size_is_dynamic")? else {
        return Err("layout size_is_dynamic is not a bool".into());
    };
    if *dynamic {
        return Err("a fixed storage application requires a fixed layout extent".into());
    }
    let size = count(field(result, "size_fixed")?, "size_fixed")?;
    let align = count(field(result, "align")?, "align")?;
    if !align.is_power_of_two() || !size.is_multiple_of(align) {
        return Err("layout size and alignment do not describe aligned fixed storage".into());
    }
    let mut offsets = vec![None; schema.len()];
    let entry_count =
        usize::try_from(entry_count).map_err(|_| "layout entry count exceeds host size")?;
    let mut entries = Vec::with_capacity(entry_count);
    let mut spans = Vec::with_capacity(entry_count);
    for cell in cells.iter().take(entry_count) {
        let key = count(field(cell, "key")?, "field key")?;
        let index = schema
            .iter()
            .position(|field| field.key == key)
            .ok_or_else(|| format!("layout refers to unknown field key {key}"))?;
        let source = &schema[index];
        let Value::Case { variant, payload } = field(cell, "representation")? else {
            return Err("layout field representation is not a FieldPlan case".into());
        };
        let variant = variant.rsplit("::").next().unwrap_or(variant);
        let offset = match (variant, payload.as_slice()) {
            ("At", [(name, value)]) if name == "offset" => count(value, "offset")?,
            _ => {
                return Err(format!(
                    "layout representation `{variant}` has no established fixed-field validation"
                ));
            }
        };
        if offsets[index].replace(offset).is_some() {
            return Err(format!("layout places field key {key} more than once"));
        }
        if !source.byte_alignment.is_power_of_two()
            || source.byte_alignment > align
            || !offset.is_multiple_of(source.byte_alignment)
        {
            return Err(format!(
                "layout placement of field key {key} violates its alignment"
            ));
        }
        let end = offset
            .checked_add(source.byte_size)
            .ok_or_else(|| format!("layout placement of field key {key} overflows"))?;
        if end > size {
            return Err(format!(
                "layout field key {key} extends past the fixed storage size"
            ));
        }
        if source.byte_size != 0 {
            spans.push((offset, end, key));
        }
        let declared = typed
            .data_members(data)
            .iter()
            .filter_map(|member| member.as_field())
            .find(|field| field.symbol == source.schema_field)
            .ok_or("layout field no longer belongs to its semantic schema")?;
        entries.push(LayoutFieldEntryReport {
            field: declared.name.to_string(),
            member_identity: source.identity,
            placement: LayoutPlacementReport::At { offset },
        });
    }
    let offsets = offsets
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("layout omits a field from its exact schema")?;
    spans.sort_unstable();
    for pair in spans.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(format!(
                "layout placements of field keys {} and {} overlap",
                pair[0].2, pair[1].2
            ));
        }
    }
    // Reports use semantic member order, independent of policy entry order.
    entries.sort_unstable_by_key(|entry| {
        schema.iter().position(|field| {
            if let Some(identity) = entry.member_identity {
                field.identity == Some(identity)
            } else {
                typed.symbols.name(field.schema_field) == entry.field
            }
        })
    });
    Ok(LayoutPlanReport {
        schema_report_fingerprint: typed_trees::identity::normalized_schema_report_fingerprint(
            typed, data,
        ),
        entries,
        offsets: Some(offsets),
        size: Some(size),
        align,
    })
}
