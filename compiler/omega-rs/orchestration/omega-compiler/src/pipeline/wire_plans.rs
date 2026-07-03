//! Derived WIRE PLANS (mint arc rung 2): every numbered schema gets a
//! placement plan -- `Varint(tag)` for scalar fields, `LengthPrefixed(tag)`
//! for text/byte-slice/nested/repeated fields -- recorded on `TypedTrees`
//! (arena + span storage) and consumed by the wire encode/decode selection,
//! which takes each field's TAG from the plan.
//!
//! RUNG 2b -- POLICIES AUTHOR THE PLAN: when the program defines a
//! `CompactBinary::plan(schema: Schema) -> Plan` machine (the Grammar law:
//! policies author plan(), everything else is derived), the compiler
//! MATERIALIZES each schema's facts (size/align/number/KIND), evaluates the
//! policy at build time (the L0 engine, purity-gated), and extracts the
//! placements from the returned Plan's FieldPlan cases. The Rust-side
//! classification below stays as the AGREEMENT ORACLE during the transition:
//! a policy whose placements diverge from the codec's walk is a compile
//! error naming both sides -- never a silent re-framing. Programs without
//! the policy keep the Rust-derived plan unchanged.
//!
//! Classification mirrors the codec's walk exactly
//! (`collect_field_appends` / `collect_field_reads`): repeated, nested, and
//! borrowed `&[u8]` fields are length-prefixed; `String` text fields are
//! length-prefixed; every other primitive is a varint scalar. Fields sort by
//! number before placement -- the emission order. A schema with a field the
//! codec cannot classify (non-primitive, negative number) gets NO plan; the
//! selection then proceeds exactly as before (its own blockers reject the
//! call), so the pass can never turn a working program into a broken one.

use omega_core::diagnostics::Diagnostic;
use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::PrimitiveType;
use omega_typed_trees::wire::{WireMember, WirePlacement};

const WIRE_GRAMMAR_POLICY: &str = "CompactBinary::plan";

/// A schema field's shape fact, mirrored to the std `FieldKind` cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldShape {
    Scalar { byte_size: i64 },
    Text,
    Nested,
    Repeated,
}

impl FieldShape {
    fn kind_case(self) -> &'static str {
        match self {
            Self::Scalar { .. } => "Scalar",
            Self::Text => "Text",
            Self::Nested => "Nested",
            Self::Repeated => "Repeated",
        }
    }

    fn is_varint(self) -> bool {
        matches!(self, Self::Scalar { .. })
    }
}

pub(crate) fn compute_wire_plans(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // Classify first (immutable walk), then record (mutable): the placement
    // arena and the schema tables cannot be borrowed simultaneously.
    let mut classified = Vec::with_capacity(typed.wire_schemas().len());
    for schema in typed.wire_schemas() {
        let mut fields: Vec<(i64, FieldShape)> = Vec::new();
        let mut classifiable = true;
        for member in typed.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.number < 0 {
                classifiable = false;
                break;
            }
            let shape = if typed.wire_field_repeated_encoding(field).is_some() {
                FieldShape::Repeated
            } else if typed.wire_field_nested_schema(field).is_some() {
                FieldShape::Nested
            } else if typed.is_borrowed_byte_slice(field.type_reference) {
                FieldShape::Text
            } else {
                match typed.primitive_type_reference(field.type_reference) {
                    Some(PrimitiveType::String) => FieldShape::Text,
                    Some(primitive) => FieldShape::Scalar {
                        byte_size: primitive_wire_size(primitive),
                    },
                    // Not a scalar the codec can varint-encode: no plan for
                    // this schema (the selection's own blockers own the
                    // rejection).
                    None => {
                        classifiable = false;
                        break;
                    }
                }
            };
            fields.push((field.number, shape));
        }
        if !classifiable {
            continue;
        }
        classified.push((schema.symbol, schema.name.as_str().to_owned(), fields));
    }

    let policy_exists = typed
        .machines()
        .iter()
        .any(|machine| machine.name.as_str() == WIRE_GRAMMAR_POLICY);

    let mut plans = Vec::with_capacity(classified.len());
    for (symbol, schema_name, fields) in classified {
        // The codec emits fields sorted by number; placements match.
        let mut derived: Vec<WirePlacement> = fields
            .iter()
            .map(|&(number, shape)| {
                if shape.is_varint() {
                    WirePlacement::Varint { tag: number }
                } else {
                    WirePlacement::LengthPrefixed { tag: number }
                }
            })
            .collect();
        derived.sort_by_key(|placement| placement.tag());

        if policy_exists {
            // THE POLICY AUTHORS THE PLAN: evaluate it against the schema's
            // materialized facts and require agreement with the codec walk.
            let authored = evaluate_wire_policy(typed, &schema_name, &fields)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if authored != derived {
                return Err(vec![Diagnostic::error(format!(
                    "wire grammar policy `{WIRE_GRAMMAR_POLICY}` disagrees with the schema \
                     walk for `{schema_name}`: the policy authored {authored:?}, the codec \
                     expects {derived:?}"
                ))]);
            }
        }

        plans.push((symbol, derived));
    }

    for (schema, placements) in plans {
        typed.record_wire_schema_plan(schema, placements);
    }
    Ok(())
}

/// Evaluate `CompactBinary::plan` (purity-gated, build-time) against the
/// schema's facts and extract the authored placements, TAG-SORTED.
fn evaluate_wire_policy(
    typed: &TypedTrees,
    schema_name: &str,
    fields: &[(i64, FieldShape)],
) -> Result<Vec<WirePlacement>, String> {
    // The purity gate: same discipline as compute_layout_plan (decision 12's
    // transitive effect surface must be empty).
    let effect_plan = omega_effects::infer_effects(typed);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == WIRE_GRAMMAR_POLICY)
        .expect("caller checked the policy exists");
    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if !transitive.is_empty() {
        return Err(format!(
            "wire grammar policy `{WIRE_GRAMMAR_POLICY}` is not effect-free: it reaches \
             effects `{}`; only effect-free machines run at build time",
            transitive.names().collect::<Vec<_>>().join(", ")
        ));
    }

    let schema_value = build_wire_schema_value(fields);
    let plan = omega_interpreter::evaluate_build_time_machine(
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

/// The wire size fact for a scalar primitive (informational for the policy;
/// varint encoding does not depend on it).
fn primitive_wire_size(primitive: PrimitiveType) -> i64 {
    match primitive {
        PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => 1,
        PrimitiveType::I16 | PrimitiveType::U16 => 2,
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => 4,
        _ => 8,
    }
}

/// Materialize the schema's facts as the std `Schema` value: field
/// (size, align, number, KIND) in declaration order, padded to 32.
fn build_wire_schema_value(fields: &[(i64, FieldShape)]) -> BuildTimeValue {
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
                ("size".to_owned(), BuildTimeValue::Int(size)),
                ("align".to_owned(), BuildTimeValue::Int(size.max(1))),
                ("number".to_owned(), BuildTimeValue::Int(number)),
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
    if *entry_count != field_count as i64 {
        return Err(fail(format!(
            "entry_count is {entry_count}, but the schema has {field_count} fields"
        )));
    }
    let BuildTimeValue::Array(cells) = field("fields")? else {
        return Err(fail("plan `fields` is not an array of FieldPlan cases".to_owned()));
    };
    fn case_name(variant: &str) -> &str {
        variant.rsplit("::").next().unwrap_or(variant)
    }
    let tag_of = |payload: &[(String, BuildTimeValue)]| -> Result<i64, String> {
        match payload.iter().find(|(name, _)| name == "tag") {
            Some((_, BuildTimeValue::Int(tag))) => Ok(*tag),
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
