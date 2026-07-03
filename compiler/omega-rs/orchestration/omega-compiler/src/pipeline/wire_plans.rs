//! Derived WIRE PLANS (mint arc rung 2a): every numbered schema gets a
//! placement plan -- `Varint(tag)` for scalar fields, `LengthPrefixed(tag)`
//! for text/byte-slice/nested/repeated fields -- recorded on `TypedTrees`
//! (arena + span storage) and consumed by the wire encode/decode selection,
//! which asserts per-field agreement with its own schema walk and takes the
//! TAG from the plan. This makes the codec PLAN-DRIVEN with byte-identical
//! output; rung 2b moves the plan's AUTHORING into an Omega `CompactBinary`
//! policy machine evaluated at build time (the plan-walking deriver proper),
//! at which point this Rust-side construction retires.
//!
//! Classification mirrors the codec's walk exactly
//! (`collect_field_appends` / `collect_field_reads`): repeated, nested, and
//! borrowed `&[u8]` fields are length-prefixed; `String` text fields are
//! length-prefixed; every other primitive is a varint scalar. Fields sort by
//! number before placement -- the emission order. A schema with a field the
//! codec cannot classify (non-primitive, negative number) gets NO plan; the
//! selection then proceeds exactly as before (its own blockers reject the
//! call), so the pass can never turn a working program into a broken one.

use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::PrimitiveType;
use omega_typed_trees::wire::{WireMember, WirePlacement};

pub(crate) fn compute_wire_plans(typed: &mut TypedTrees) {
    // Classify first (immutable walk), then record (mutable): the placement
    // arena and the schema tables cannot be borrowed simultaneously.
    let mut plans = Vec::with_capacity(typed.wire_schemas().len());
    for schema in typed.wire_schemas() {
        let mut placements: Vec<WirePlacement> = Vec::new();
        let mut classifiable = true;
        for member in typed.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.number < 0 {
                classifiable = false;
                break;
            }
            let length_prefixed = typed.wire_field_repeated_encoding(field).is_some()
                || typed.wire_field_nested_schema(field).is_some()
                || typed.is_borrowed_byte_slice(field.type_reference)
                || matches!(
                    typed.primitive_type_reference(field.type_reference),
                    Some(PrimitiveType::String)
                );
            if !length_prefixed && typed.primitive_type_reference(field.type_reference).is_none() {
                // Not a scalar the codec can varint-encode: no plan for this
                // schema (the selection's own blockers own the rejection).
                classifiable = false;
                break;
            }
            placements.push(if length_prefixed {
                WirePlacement::LengthPrefixed { tag: field.number }
            } else {
                WirePlacement::Varint { tag: field.number }
            });
        }
        if !classifiable {
            continue;
        }
        // The codec emits fields sorted by number; placements match.
        placements.sort_by_key(|placement| placement.tag());
        plans.push((schema.symbol, placements));
    }

    for (schema, placements) in plans {
        typed.record_wire_schema_plan(schema, placements);
    }
}
