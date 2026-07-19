//! Programmable-layout evaluation and validation.
//!
//! The compiler materializes `Schema`, invokes an effect-free policy machine
//! at build time, and validates the returned `Plan` before any consumer trusts
//! it. Plans are keyed by compiler-issued field identities rather than array
//! position, so policies may reorder entries or fragment one logical field.

use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::PrimitiveType;

const SCHEMA_FIELD_CAPACITY: usize = 32;
const PLAN_ENTRY_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutPlacementReport {
    At {
        offset: i64,
    },
    Bits {
        container: i64,
        container_width: i64,
        destination_lsb: i64,
        source_lsb: i64,
        width: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldEntryReport {
    /// Normalized field name. Compiler-issued keys do not escape into artifact
    /// reports or identity.
    pub field: String,
    pub placement: LayoutPlacementReport,
}

/// A validated layout plan, ready for consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlanReport {
    pub entries: Vec<LayoutFieldEntryReport>,
    /// Declaration-order offsets when every field has one fixed `At`
    /// placement. Fragmented plans deliberately have no such projection.
    pub offsets: Option<Vec<i64>>,
    pub size: Option<i64>,
    pub align: i64,
}

#[derive(Debug, Clone)]
struct SchemaFieldInfo {
    name: String,
    key: i64,
    size: i64,
    align: i64,
}

pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<LayoutPlanReport, String> {
    let schema_fields = schema_fields(typed, schema_data)?;
    let schema_value = build_schema_value(&schema_fields);

    let effect_plan = omega_effects::infer_effects(typed);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if !transitive.is_empty() {
        return Err(format!(
            "policy machine `{policy_machine}` is not effect-free: it reaches effects `{}`; \
             only effect-free machines run at build time",
            transitive.names().collect::<Vec<_>>().join(", ")
        ));
    }

    let plan =
        omega_interpreter::evaluate_build_time_machine(typed, policy_machine, vec![schema_value])
            .map_err(|reason| {
            format!("build-time evaluation of `{policy_machine}` failed: {reason}")
        })?;

    validate_plan(&plan, &schema_fields, policy_machine)
}

fn schema_fields(typed: &TypedTrees, schema_data: &str) -> Result<Vec<SchemaFieldInfo>, String> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;

    let mut fields = Vec::new();
    for member in typed.data_members(data) {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return Err(format!(
                "schema data `{schema_data}` has a case member; the current layout slice supports plain struct fields only"
            ));
        };
        let Some(primitive) = typed.primitive_type_reference(field.type_reference) else {
            return Err(format!(
                "schema data `{schema_data}` field `{}` is not a primitive; the current layout slice supports primitive fields only",
                field.name
            ));
        };
        let size = primitive_byte_size(primitive).ok_or_else(|| {
            format!(
                "schema data `{schema_data}` field `{}` has type `{}`, which the current layout slice cannot size",
                field.name,
                primitive.name()
            )
        })?;
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
            key,
            size,
            align: size,
        });
    }
    if fields.is_empty() {
        return Err(format!("schema data `{schema_data}` has no fields"));
    }
    if fields.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} fields; the current layout slice supports at most {}",
            fields.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    Ok(fields)
}

fn primitive_byte_size(primitive: PrimitiveType) -> Option<i64> {
    Some(match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
        PrimitiveType::I16 | PrimitiveType::U16 => 2,
        PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32 => 4,
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 => 8,
        _ => return None,
    })
}

fn field_key(schema: &str, field: &str) -> i64 {
    // Stable FNV-1a. Zero remains the unused-tail sentinel.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in schema.bytes().chain([b':', b':']).chain(field.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let key = hash as i64;
    if key == 0 { 1 } else { key }
}

fn build_schema_value(schema_fields: &[SchemaFieldInfo]) -> BuildTimeValue {
    let mut fields = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
    for index in 0..SCHEMA_FIELD_CAPACITY {
        let (key, size, align) = schema_fields
            .get(index)
            .map(|field| (field.key, field.size, field.align))
            .unwrap_or((0, 0, 1));
        fields.push(BuildTimeValue::Struct {
            type_name: "SchemaField".to_owned(),
            fields: vec![
                ("key".to_owned(), BuildTimeValue::Int(key)),
                ("size".to_owned(), BuildTimeValue::Int(size)),
                ("align".to_owned(), BuildTimeValue::Int(align)),
                ("number".to_owned(), BuildTimeValue::Int(-1)),
                (
                    "kind".to_owned(),
                    BuildTimeValue::Case {
                        variant: "Scalar".to_owned(),
                        payload: Vec::new(),
                    },
                ),
            ],
        });
    }
    BuildTimeValue::Struct {
        type_name: "Schema".to_owned(),
        fields: vec![
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            (
                "field_count".to_owned(),
                BuildTimeValue::Int(schema_fields.len() as i64),
            ),
        ],
    }
}

fn validate_plan(
    plan: &BuildTimeValue,
    schema_fields: &[SchemaFieldInfo],
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
    let int_field = |name: &str| -> Result<i64, String> {
        match field(name)? {
            BuildTimeValue::Int(value) => Ok(*value),
            other => Err(fail(format!(
                "plan field `{name}` is not an integer: {other:?}"
            ))),
        }
    };

    let entry_count = int_field("entry_count")?;
    if entry_count < 0 || entry_count as usize > PLAN_ENTRY_CAPACITY {
        return Err(fail(format!(
            "entry_count {entry_count} is outside 0..={PLAN_ENTRY_CAPACITY}"
        )));
    }
    let size_is_dynamic = match field("size_is_dynamic")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(fail(format!("size_is_dynamic is not a bool: {other:?}"))),
    };
    let size_fixed = int_field("size_fixed")?;
    let align = int_field("align")?;
    if align < 1 || (align & (align - 1)) != 0 {
        return Err(fail(format!(
            "alignment {align} is not a positive power of two"
        )));
    }
    let BuildTimeValue::Array(entry_cells) = field("entries")? else {
        return Err(fail(
            "plan `entries` is not an array of FieldEntry values".to_owned(),
        ));
    };
    if entry_count as usize > entry_cells.len() {
        return Err(fail(format!(
            "entry_count is {entry_count}, but the plan carries only {} entries",
            entry_cells.len()
        )));
    }

    let case_name = |variant: &str| variant.rsplit("::").next().unwrap_or(variant).to_owned();
    let lookup = |key: i64| schema_fields.iter().position(|field| field.key == key);
    let payload_int = |payload: &[(String, BuildTimeValue)], name: &str| -> Result<i64, String> {
        match payload.iter().find(|(field, _)| field == name) {
            Some((_, BuildTimeValue::Int(value))) => Ok(*value),
            other => Err(fail(format!(
                "placement carries no integer `{name}`: {other:?}"
            ))),
        }
    };

    let mut entries = Vec::with_capacity(entry_count as usize);
    let mut source_spans: Vec<Vec<(i64, i64)>> = vec![Vec::new(); schema_fields.len()];
    let mut destination_spans: Vec<(i64, i64, String)> = Vec::new();
    let mut offsets_by_field = vec![None; schema_fields.len()];
    let mut kinds_by_field: Vec<Option<&'static str>> = vec![None; schema_fields.len()];

    for entry_index in 0..entry_count as usize {
        let Some(BuildTimeValue::Struct { fields, .. }) = entry_cells.get(entry_index) else {
            return Err(fail(format!(
                "entry {entry_index} is missing or not a FieldEntry"
            )));
        };
        let key = match fields.iter().find(|(name, _)| name == "key") {
            Some((_, BuildTimeValue::Int(key))) => *key,
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
                if kinds_by_field[field_index].replace("At").is_some() {
                    return Err(fail(format!(
                        "field `{}` has more than one placement",
                        schema_field.name
                    )));
                }
                let offset = payload_int(payload, "offset")?;
                if offset < 0 {
                    return Err(fail(format!(
                        "field `{}` is placed at negative offset {offset}",
                        schema_field.name
                    )));
                }
                if offset % schema_field.align != 0 {
                    return Err(fail(format!(
                        "field `{}` at offset {offset} violates its alignment {}",
                        schema_field.name, schema_field.align
                    )));
                }
                let end = offset.checked_add(schema_field.size).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let start_bit = offset.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                let end_bit = end.checked_mul(8).ok_or_else(|| {
                    fail(format!("field `{}` placement overflows", schema_field.name))
                })?;
                destination_spans.push((start_bit, end_bit, schema_field.name.clone()));
                offsets_by_field[field_index] = Some(offset);
                entries.push(LayoutFieldEntryReport {
                    field: schema_field.name.clone(),
                    placement: LayoutPlacementReport::At { offset },
                });
            }
            "Bits" => {
                if matches!(kinds_by_field[field_index], Some("At")) {
                    return Err(fail(format!(
                        "field `{}` mixes `At` and `Bits` placements",
                        schema_field.name
                    )));
                }
                kinds_by_field[field_index] = Some("Bits");
                let container = payload_int(payload, "container")?;
                let container_width = payload_int(payload, "container_width")?;
                let destination_lsb = payload_int(payload, "destination_lsb")?;
                let source_lsb = payload_int(payload, "source_lsb")?;
                let width = payload_int(payload, "width")?;
                if container < 0
                    || container_width <= 0
                    || destination_lsb < 0
                    || source_lsb < 0
                    || width <= 0
                {
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
                if source_end > schema_field.size * 8 {
                    return Err(fail(format!(
                        "field `{}` fragment source {}..{} exceeds its {}-bit value",
                        schema_field.name,
                        source_lsb,
                        source_end,
                        schema_field.size * 8
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
                if cursor != schema_field.size * 8 {
                    return Err(fail(format!(
                        "field `{}` source fragments end at bit {cursor}, expected {}",
                        schema_field.name,
                        schema_field.size * 8
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
        if size_fixed < 0 {
            return Err(fail(format!("fixed size {size_fixed} is negative")));
        }
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
            LayoutPlacementReport::At { .. } => 0,
            LayoutPlacementReport::Bits { source_lsb, .. } => *source_lsb,
        };
        (field_index, source_lsb)
    });

    let offsets = offsets_by_field
        .iter()
        .all(Option::is_some)
        .then(|| offsets_by_field.into_iter().map(Option::unwrap).collect());

    Ok(LayoutPlanReport {
        entries,
        offsets,
        size: (!size_is_dynamic).then_some(size_fixed),
        align,
    })
}
