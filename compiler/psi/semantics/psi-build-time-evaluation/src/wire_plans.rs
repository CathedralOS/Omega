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
//! policy at build time (the L0 engine, contract-gated), and extracts the
//! placements from the returned Plan's FieldPlan cases. The Rust-side
//! classification below stays as the AGREEMENT ORACLE during the transition:
//! a policy whose placements diverge from the codec's walk is a compile
//! error naming both sides -- never a silent re-framing. Programs without
//! the policy keep the Rust-derived plan unchanged.
//!
//! Classification mirrors the codec's walk exactly
//! (`collect_field_appends` / `collect_field_reads`): repeated, nested, and
//! borrowed `&[u8]` text fields are length-prefixed; every other primitive is
//! a varint scalar. Fields sort by
//! number before placement -- the emission order. A schema with a field the
//! codec cannot classify (non-primitive, negative number) gets NO plan; the
//! selection then proceeds exactly as before (its own blockers reject the
//! call), so the pass can never turn a working program into a broken one.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::PrimitiveType;
use psi_typed_trees::wire::{WireMember, WirePlacement};

mod policy_value;

use policy_value::evaluate_wire_policy;

const WIRE_GRAMMAR_POLICY: &str = "CompactBinary::plan";

/// A schema field's shape fact, mirrored to the std `FieldKind` cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldShape {
    Scalar {
        byte_size: u64,
    },
    Text,
    Nested,
    Repeated,
    BorrowedScalarSlice {
        element: psi_typed_trees::wire::WireScalarEncoding,
    },
}

impl FieldShape {
    fn kind_case(self) -> &'static str {
        match self {
            Self::Scalar { .. } => "Scalar",
            Self::Text => "Text",
            Self::Nested => "Nested",
            Self::Repeated => "Repeated",
            Self::BorrowedScalarSlice { .. } => "Repeated",
        }
    }

    fn is_varint(self) -> bool {
        matches!(self, Self::Scalar { .. })
    }
}

pub fn compute_wire_plans(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // Classify first (immutable walk), then record (mutable): the placement
    // arena and the schema tables cannot be borrowed simultaneously.
    let mut classified = Vec::with_capacity(typed.wire_schemas().len());
    for schema in typed.wire_schemas() {
        let mut fields: Vec<(u64, FieldShape)> = Vec::new();
        let mut classifiable = true;
        for member in typed.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let shape = if typed.wire_field_repeated_encoding(field).is_some() {
                FieldShape::Repeated
            } else if typed.wire_field_nested_schema(field).is_some() {
                FieldShape::Nested
            } else if typed.is_borrowed_byte_slice(field.type_reference) {
                FieldShape::Text
            } else if let Some(slice) = typed.wire_field_borrowed_scalar_slice_encoding(field) {
                FieldShape::BorrowedScalarSlice {
                    element: slice.element,
                }
            } else {
                match typed.primitive_type_reference(field.type_reference) {
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
    let admission = policy_exists.then(|| crate::BuildTimeAdmissionPlan::infer(typed));

    let mut plans = Vec::with_capacity(classified.len());
    for (symbol, schema_name, fields) in classified {
        // The codec emits fields sorted by number; placements match.
        let mut derived: Vec<WirePlacement> = fields
            .iter()
            .map(|&(number, shape)| {
                let tag = u64::try_from(number)
                    .expect("classifiable wire fields have nonnegative identity numbers");
                if shape.is_varint() {
                    WirePlacement::Varint { tag }
                } else {
                    WirePlacement::LengthPrefixed { tag }
                }
            })
            .collect();
        derived.sort_by_key(|placement| placement.tag());

        if policy_exists {
            // THE POLICY AUTHORS THE PLAN: evaluate it against the schema's
            // materialized facts and require agreement with the codec walk.
            let authored = evaluate_wire_policy(
                typed,
                admission
                    .as_ref()
                    .expect("a policy admission plan exists with the policy"),
                &schema_name,
                &fields,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if authored != derived {
                return Err(vec![Diagnostic::error(format!(
                    "wire grammar policy `{WIRE_GRAMMAR_POLICY}` disagrees with the schema \
                     walk for `{schema_name}`: the policy authored {authored:?}, the codec \
                     expects {derived:?}"
                ))]);
            }
        }

        let obligations = fields
            .iter()
            .filter_map(|&(field_number, shape)| {
                let FieldShape::BorrowedScalarSlice { element } = shape else {
                    return None;
                };
                Some(psi_typed_trees::wire::WireEncodeObligation {
                    field_number,
                    element,
                    length:
                        psi_typed_trees::wire::WireEncodeLengthObligation::RuntimeElementCount,
                    work:
                        psi_typed_trees::wire::WireEncodeWorkObligation::TwoPassesPerElement,
                    output_capacity:
                        psi_typed_trees::wire::WireEncodeOutputCapacityObligation::ExactPackedPayload,
                })
            })
            .collect::<Vec<_>>();
        plans.push((symbol, derived, obligations));
    }

    for (schema, placements, obligations) in plans {
        typed.record_wire_schema_plan(schema, placements, obligations);
    }
    Ok(())
}

/// The wire size fact for a scalar primitive (informational for the policy;
/// varint encoding does not depend on it). Non-scalars report 8,
/// preserving the prior `_ => 8` fallback. Sizes come from the single source of
/// truth on `PrimitiveType`.
fn primitive_wire_size(primitive: PrimitiveType) -> u64 {
    primitive.scalar_byte_size().unwrap_or(8) as u64
}
