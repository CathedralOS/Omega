//! Physical inventories of source places, including builtin collection windows.
//!
//! A window has no invented fixed-array type. Its declared element inventory
//! is restricted to its proved extent; source leaf validity and observation
//! protocols remain on that inventory. This query grants no borrow authority.

use super::record_representation::{
    MutableRecordRepresentation, array_region_representation, type_representation,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

/// Declaration roots used to select every physical layout alternative. A
/// range itself has no type-table row, but its collection still owns its
/// element's selected representation. This carries no range/bounds proof.
pub(super) fn source_layout_root(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(operation) = crate::value_custody::places::subslice_expression(program, source) {
        let indexed = program.expression_table.indexed_node(operation)?;
        crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            indexed.collection,
        )
    } else {
        crate::value_custody::places::declared_place_type_raw(program, machine, Some(state), source)
    }
}

pub(super) fn source_representation(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> Result<MutableRecordRepresentation, &'static str> {
    let places = crate::value_custody::places::declared_place_type_raw;
    let Some(operation) = crate::value_custody::places::subslice_expression(program, source) else {
        let reference = places(program, machine, Some(state), source)
            .ok_or("recast source has no declaration-owned storage type")?;
        return type_representation(program, reference)
            .ok_or("recast source has no fixed representation inventory");
    };
    if !crate::value_custody::places::has_builtin_subslice_meaning(
        program,
        machine,
        Some(state),
        operation,
    ) {
        return Err("recast source range has no builtin storage correspondence");
    }
    let indexed = program.expression_table.indexed_node(operation).unwrap();
    let range = program.expression_table.range_node(indexed.index).unwrap();
    let collection = places(program, machine, Some(state), indexed.collection)
        .ok_or("recast source window has no declaration-owned collection")?;
    let collection = crate::value_custody::places::assignment_value_type(program, collection);
    if matches!(
        program.type_reference_table.type_reference(collection),
        TypeReferenceNode::Constrained { .. }
    ) {
        return Err("recast source window must retain its collection's qualifications");
    }
    let TypeReferenceNode::FixedArray {
        length: FixedArrayLength::Literal(extent),
        ..
    } = program.type_reference_table.type_reference(collection)
    else {
        return Err("recast source window needs a proved fixed collection extent");
    };
    let coordinate = |expression| {
        program
            .closed_integer_expression_value_in(expression, machine.symbol)
            .and_then(|value| value.to_u64())
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("recast source window endpoint is not statically exact")
    };
    let start = if range.start.is_valid() {
        coordinate(range.start)?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        let end = coordinate(range.end)?;
        if range.end_inclusive {
            end.checked_add(1)
                .ok_or("recast source window endpoint overflows")?
        } else {
            end
        }
    } else if range.end_inclusive {
        return Err("an inclusive recast source window needs an end coordinate");
    } else {
        *extent
    };
    if start > end || end > *extent {
        return Err("recast source window lies outside its declared collection extent");
    }
    // Fixed arrays use the element's padded size as their stride. A source
    // element already owes alignment, so every admitted interior start retains
    // that alignment. The selected inventory includes the complete window.
    array_region_representation(program, collection, start, end)
        .ok_or("recast source window has no bounded fixed representation inventory")
}
