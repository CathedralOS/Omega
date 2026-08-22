use super::offset_bounds::incoming_guard_offset_bound;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::HashSet;

/// Exact value of an interior byte offset when its syntax or declared range
/// pins one value. A mere upper bound is sufficient for fixed-footprint views,
/// but an unsized slice with multi-byte elements additionally owes divisibility
/// of the complete remaining region; an interval cannot prove that congruence.
pub(super) fn exact_interior_byte_region_offset(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(indexed.index) {
        return literal.value_i64().filter(|offset| *offset >= 0);
    }
    let type_reference =
        crate::places::declared_place_type_raw(program, machine, Some(state), indexed.index)?;
    let interval = crate::arithmetic_domains::range_constraint_interval(program, type_reference)?;
    let (Some(low), Some(high)) = (interval.low(), interval.high()) else {
        return None;
    };
    (low == high && low >= 0).then_some(low)
}

/// The interior byte-region judgment's three-way answer (owner-measured
/// diagnostic split 2026-07-11: a recognized shape whose OFFSET cannot be
/// bounded must say so -- it used to fall through to the form errors
/// ("not a scalar primitive or an eligible fixed record" / "source must be a
/// borrowed scalar place"), which misled: the real failure was the unproven
/// bound).
pub(super) enum InteriorByteRegion {
    /// Not `<[u8; N] place>[k]` at all -- fall through to the other source
    /// classes and their form messages.
    NotInteriorShape,
    /// The shape is right, but no route bounds the runtime offset.
    OffsetUnproven {
        offset_display: String,
        region_length: i64,
    },
    /// `k` (or its proven upper bound) and `N`.
    Bounded { offset: i64, region_length: i64 },
}

/// Rungs B/C1's interior source: `<[u8; N] place>[k]`. Shape is recognized
/// FIRST (byte-element fixed array, literal length); the offset bound then
/// comes from a literal, the declared range, the dominating incoming
/// guard, or the boundary-ensures witness.
pub(super) fn interior_byte_region_source(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    source: ExpressionHandle,
) -> InteriorByteRegion {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let Some(collection_type) =
        crate::places::declared_place_type(program, machine, Some(state), indexed.collection)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
        ..
    } = program.type_reference_table.type_reference(collection_type)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let element_is_byte = crate::places::unwrapped_type_reference(program, *element_type)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
        == Some(PrimitiveType::U8);
    if !element_is_byte {
        return InteriorByteRegion::NotInteriorShape;
    }
    let psi_typed_trees::types::FixedArrayLength::Literal(length) = length else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let region_length = *length as i64;

    // RUNG C1: a RUNTIME offset (`&self.buf[k] as &u32`) discharges through
    // the index place's enforced interval -- its declared range (dependent
    // maxima substitute through the field's own range) bounds the offset,
    // so `high(k) + size(T) <= N` is the footprint check. The interval is
    // store-enforced/caller-proved by the R1 machinery, so it is a true
    // bound at every read. Gap #4 routes: the dominating incoming-arm
    // guard, and the R4 boundary-ensures witness.
    let offset = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Integer(literal) => literal.value_i64().filter(|offset| *offset >= 0),
        _ => {
            let declared_high = crate::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.index,
            )
            .and_then(|raw| {
                let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
                let high = interval.high()?;
                (!interval.low().is_some_and(|low| low < 0) && high >= 0).then_some(high)
            });
            declared_high
                .or_else(|| incoming_guard_offset_bound(program, machine, state, indexed.index))
        }
    };
    match offset {
        Some(offset) => InteriorByteRegion::Bounded {
            offset,
            region_length,
        },
        None => InteriorByteRegion::OffsetUnproven {
            offset_display: program.expression_table.display_name(indexed.index),
            region_length,
        },
    }
}

pub(super) fn push_offset_unproven(
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    offset_display: &str,
    region_length: i64,
) {
    diagnostics.push(Diagnostic::error(format!(
        "{context}: cannot bound the recast offset `{offset_display}` -- the region holds \
         {region_length} bytes, but no declared range, dominating incoming guard, or \
         boundary-ensures witness bounds the offset below the footprint. Bound it: declare \
         a range on the offset param, guard the transition arm (`transition \
         {offset_display} <= K {{ true -> ... }}`), or `ensures`-bound the boundary \
         out-param that feeds it",
    )));
}

/// Mutable byte views must preserve every target fact after arbitrary writes.
/// Until the general bidirectional entailment solver lands, the complete
/// decidable subset is a raw named record whose transitive fields are raw
/// fact-free scalar primitives or records with no default-domain facts.
fn record_view_is_fact_free(
    program: &TypedTrees,
    name: &str,
    visiting: &mut HashSet<String>,
) -> bool {
    if !visiting.insert(name.to_owned()) {
        return false;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
    else {
        visiting.remove(name);
        return false;
    };
    if !data.where_facts.is_empty() || data.zero_gated {
        visiting.remove(name);
        return false;
    }
    for member in program.data_members(data) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            visiting.remove(name);
            return false;
        };
        if !record_view_type_is_fact_free(program, field.type_reference, visiting) {
            visiting.remove(name);
            return false;
        }
    }
    visiting.remove(name);
    true
}

pub(super) fn record_view_type_is_fact_free(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            PrimitiveType::from_name(name.as_str())
                .is_some_and(|primitive| primitive != PrimitiveType::Bool)
                || record_view_is_fact_free(program, name.as_str(), visiting)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => record_view_type_is_fact_free(program, *element_type, visiting),
        _ => false,
    }
}
