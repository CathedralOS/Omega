//! L2/L3 of the LAYOUTS ladder (design_briefs/programmable_layouts.md): the
//! compiler materializes a `Schema` value from a data definition, invokes a
//! policy's `plan()` machine at BUILD TIME (through the reference
//! interpreter -- no keyword; decision 12's transitive effect surface is the
//! legality gate), then VALIDATES the returned `Plan` before anything trusts
//! it. A malicious or buggy policy is a compile error, never unsafety: the
//! validation owns soundness, so policies are free to compute however they
//! like internally.
//!
//! v0 slice: primitive-typed fields only (the pilot's C-layout client),
//! index-keyed flat plans (see omega/language/std/layout.omg for the shape
//! and the target vocabulary it stands in for).

use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::PrimitiveType;

/// The maximum schema width of the v0 slice (the `[SchemaField; 32]` shape).
const SCHEMA_FIELD_CAPACITY: usize = 32;

/// A validated layout plan, ready for consumers (the report artifact today;
/// derived projections/codecs up-ladder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlanReport {
    /// Byte offset per schema field, in declaration order.
    pub offsets: Vec<i64>,
    /// Total size when the plan is fixed; `None` for dynamic plans.
    pub size: Option<i64>,
    pub align: i64,
}

/// Evaluate `policy_machine` (e.g. `CLayout::plan`) against the fields of the
/// plain data definition `schema_data`, validate the plan, and report it.
pub fn compute_layout_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
) -> Result<LayoutPlanReport, String> {
    let field_sizes = schema_field_sizes(typed, schema_data)?;
    let schema_value = build_schema_value(&field_sizes);

    // The purity gate: decision 12's inferred TRANSITIVE effect surface must
    // be empty (the same plan validate_pure_discards consults). By-value
    // arguments are guaranteed by the call shape (we build the value).
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

    let plan = omega_interpreter::evaluate_build_time_machine(
        typed,
        policy_machine,
        vec![schema_value],
    )
    .map_err(|reason| format!("build-time evaluation of `{policy_machine}` failed: {reason}"))?;

    validate_plan(&plan, &field_sizes, policy_machine)
}

/// The (size, align) of each field of the schema data definition, in
/// declaration order. v0: primitive integer/bool/float fields only.
fn schema_field_sizes(typed: &TypedTrees, schema_data: &str) -> Result<Vec<(i64, i64)>, String> {
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == schema_data)
        .ok_or_else(|| format!("no data definition named `{schema_data}` exists"))?;

    let mut sizes = Vec::new();
    for member in typed.data_members(data) {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return Err(format!(
                "schema data `{schema_data}` has a case member; the v0 layout slice supports \
                 plain struct fields only"
            ));
        };
        let Some(primitive) = typed.primitive_type_reference(field.type_reference) else {
            return Err(format!(
                "schema data `{schema_data}` field `{}` is not a primitive; the v0 layout slice \
                 supports primitive fields only",
                field.name
            ));
        };
        let size = primitive_byte_size(primitive).ok_or_else(|| {
            format!(
                "schema data `{schema_data}` field `{}` has type `{}`, which the v0 layout \
                 slice cannot size",
                field.name,
                primitive.name()
            )
        })?;
        sizes.push((size, size)); // natural alignment == size for primitives
    }
    if sizes.is_empty() {
        return Err(format!("schema data `{schema_data}` has no fields"));
    }
    if sizes.len() > SCHEMA_FIELD_CAPACITY {
        return Err(format!(
            "schema data `{schema_data}` has {} fields; the v0 layout slice supports at most {}",
            sizes.len(),
            SCHEMA_FIELD_CAPACITY
        ));
    }
    Ok(sizes)
}

fn primitive_byte_size(primitive: PrimitiveType) -> Option<i64> {
    Some(match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
        PrimitiveType::I16 | PrimitiveType::U16 => 2,
        PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32 => 4,
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 | PrimitiveType::Usize => 8,
        _ => return None,
    })
}

/// Materialize the `Schema` argument value: field (size, align, number) in
/// declaration order, tail slots zeroed (entry positions past `field_count`
/// are meaningless by contract).
fn build_schema_value(field_sizes: &[(i64, i64)]) -> BuildTimeValue {
    let mut fields = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
    for index in 0..SCHEMA_FIELD_CAPACITY {
        let (size, align) = field_sizes.get(index).copied().unwrap_or((0, 1));
        fields.push(BuildTimeValue::Struct {
            type_name: "SchemaField".to_owned(),
            fields: vec![
                ("size".to_owned(), BuildTimeValue::Int(size)),
                ("align".to_owned(), BuildTimeValue::Int(align)),
                ("number".to_owned(), BuildTimeValue::Int(-1)),
            ],
        });
    }
    BuildTimeValue::Struct {
        type_name: "Schema".to_owned(),
        fields: vec![
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            (
                "field_count".to_owned(),
                BuildTimeValue::Int(field_sizes.len() as i64),
            ),
        ],
    }
}

/// L2: decode and VALIDATE the returned plan. Checks: entry count matches the
/// schema, offsets non-negative, no overlap between placed fields, every
/// field in-bounds of a fixed size, alignment is a positive power of two, and
/// the fixed size is aligned. A failed check is a compile-time error naming
/// the policy.
fn validate_plan(
    plan: &BuildTimeValue,
    field_sizes: &[(i64, i64)],
    policy_machine: &str,
) -> Result<LayoutPlanReport, String> {
    let fail = |reason: String| format!("policy `{policy_machine}` produced an invalid plan: {reason}");

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
            other => Err(fail(format!("plan field `{name}` is not an integer: {other:?}"))),
        }
    };

    let entry_count = int_field("entry_count")?;
    if entry_count != field_sizes.len() as i64 {
        return Err(fail(format!(
            "entry_count is {entry_count}, but the schema has {} fields",
            field_sizes.len()
        )));
    }
    let size_is_dynamic = match field("size_is_dynamic")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(fail(format!("size_is_dynamic is not a bool: {other:?}"))),
    };
    let size_fixed = int_field("size_fixed")?;
    let align = int_field("align")?;
    if align < 1 || (align & (align - 1)) != 0 {
        return Err(fail(format!("alignment {align} is not a positive power of two")));
    }

    let BuildTimeValue::Array(offset_cells) = field("offsets")? else {
        return Err(fail("plan `offsets` is not an array".to_owned()));
    };
    let mut offsets = Vec::with_capacity(field_sizes.len());
    for (index, _) in field_sizes.iter().enumerate() {
        match offset_cells.get(index) {
            Some(BuildTimeValue::Int(offset)) => offsets.push(*offset),
            other => {
                return Err(fail(format!(
                    "offset entry {index} is missing or non-integer: {other:?}"
                )));
            }
        }
    }

    // Placement checks: non-negative, aligned, non-overlapping, in-bounds.
    let mut spans: Vec<(i64, i64, usize)> = Vec::new();
    for (index, (&offset, &(size, field_align))) in
        offsets.iter().zip(field_sizes.iter()).enumerate()
    {
        if offset < 0 {
            return Err(fail(format!("field {index} is placed at negative offset {offset}")));
        }
        if field_align > 0 && offset % field_align != 0 {
            return Err(fail(format!(
                "field {index} at offset {offset} violates its alignment {field_align}"
            )));
        }
        spans.push((offset, offset + size, index));
    }
    spans.sort();
    for pair in spans.windows(2) {
        let (_, end_a, index_a) = pair[0];
        let (start_b, _, index_b) = pair[1];
        if start_b < end_a {
            return Err(fail(format!(
                "fields {index_a} and {index_b} overlap"
            )));
        }
    }
    if !size_is_dynamic {
        if let Some(&(_, end, index)) = spans.last() {
            if end > size_fixed {
                return Err(fail(format!(
                    "field {index} ends at {end}, past the fixed size {size_fixed}"
                )));
            }
        }
        if size_fixed % align != 0 {
            return Err(fail(format!(
                "fixed size {size_fixed} is not a multiple of the alignment {align}"
            )));
        }
    }

    Ok(LayoutPlanReport {
        offsets,
        size: (!size_is_dynamic).then_some(size_fixed),
        align,
    })
}
